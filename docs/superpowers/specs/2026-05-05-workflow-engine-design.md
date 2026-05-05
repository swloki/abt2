---
name: Workflow Engine Design
date: 2026-05-05
status: approved
---

# Workflow Engine Design

## Overview

为 ABT 系统引入嵌入式工作流引擎，支持可配置的审批流程和任务分配。引擎完全集成在现有 Rust + PostgreSQL 架构中，不依赖外部服务。

## Decision: Self-built vs External Library

选择自建引擎，排除 Temporal 等外部工作流平台，原因：

- Temporal Rust SDK 仍为 prerelease，API 不稳定
- Temporal 需要独立的 Temporal Server 部署，与"嵌入式"需求冲突
- ABT 的工作流需求（审批 + 任务分配）复杂度可控，不需要分布式编排能力
- 自建方案保持所有数据在同一个 PostgreSQL 中，事务一致性更易保证

## Architecture

遵循现有分层模式：

```
proto/abt/v1/workflow.proto           ← gRPC 定义
abt/src/models/workflow.rs            ← 数据模型（含 Condition AST）
abt/src/repositories/
  workflow_template_repo.rs           ← 模板 CRUD
  workflow_instance_repo.rs           ← 实例 CRUD
  workflow_task_repo.rs               ← 任务 CRUD
abt/src/service/workflow_service.rs   ← Service trait
abt/src/implt/workflow_engine.rs      ← 核心引擎
abt/src/implt/workflow_hooks.rs       ← WorkflowHook trait + 各实体的 hook 实现
abt-grpc/src/handlers/workflow.rs     ← gRPC handler
```

## Design Improvements (from Ideation)

基于 ideation 分析，采纳以下 5 项改进：

### Improvement 1: 3 表设计替代 5 表（折叠 nodes + edges 到 JSONB）

将 `workflow_nodes` 和 `workflow_edges` 折叠为 `workflow_templates` 上的一个 JSONB 列 `graph`。V1 没有可视化设计器，流程图很小（3-8 节点），模板是原子发布的。这消除了 2 个 repository、2 组 CRUD 接口和跨表 JOIN。

### Improvement 2: 冻结图快照

实例创建时将完整图定义序列化到 `frozen_graph` JSONB 列。运行时无需 JOIN 模板表，模板编辑不影响运行中的实例。与 BOM 系统的快照模式一致（`bom_nodes` 在创建时拷贝子节点结构）。

### Improvement 3: 移除 current_node_ids 数组

当前活跃节点从 `workflow_tasks WHERE status = 'pending'` 派生查询得出，不存储冗余的可变数组。消除并行网关合并时的行级锁争用。

### Improvement 4: Condition AST 替代原始 JSONB 条件

用 Rust enum 表达式树（`Condition::And`, `Condition::FieldCompare`, `Condition::Or` 等）替代原始 JSONB 条件。条件在模板创建时验证，防止类型不匹配导致的静默路由失败（与产品表 `meta->>'category'` 的 bug 同源）。

### Improvement 5: Fail-closed assignee + 升级路径

assignee 规则解析为零候选人时，fail-closed（暂停流程 + 报警），不跳过审批。每个节点配置必须包含 `fallback_assignee` 防止永久阻塞。

## Data Model

### workflow_templates（含折叠的图定义）

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| name | VARCHAR | 模板名称 |
| entity_type | VARCHAR | 关联业务实体类型（product, bom, purchase_order 等） |
| version | INT | 版本号 |
| status | VARCHAR | draft / active / archived |
| graph | JSONB | 图定义：`{nodes: [...], edges: [...]}`，包含完整的节点和边 |
| created_at | TIMESTAMP | |
| updated_at | TIMESTAMP | |

#### graph JSONB 结构

```json
{
  "nodes": [
    {
      "id": "start",
      "node_type": "start",
      "name": "开始"
    },
    {
      "id": "manager_approval",
      "node_type": "approval",
      "name": "主管审批",
      "config": {
        "assignee_type": "role",
        "assignee_value": "manager",
        "multi_approval": "any",
        "reject_action": "terminate",
        "fallback_assignee": "admin"
      }
    },
    {
      "id": "end",
      "node_type": "end",
      "name": "结束"
    }
  ],
  "edges": [
    {
      "from": "start",
      "to": "manager_approval"
    },
    {
      "from": "manager_approval",
      "to": "end",
      "condition": {"FieldCompare": {"field": "amount", "op": "LtEq", "value": 10000}}
    },
    {
      "from": "manager_approval",
      "to": "cfo_approval",
      "condition": {"FieldCompare": {"field": "amount", "op": "Gt", "value": 10000}}
    }
  ]
}
```

### workflow_instances（含冻结图快照）

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| template_id | UUID | FK → templates（仅用于追溯，运行时不 JOIN） |
| template_version | INT | 创建时的模板版本号 |
| entity_type | VARCHAR | 业务实体类型 |
| entity_id | UUID | 业务实体 ID |
| status | VARCHAR | running / completed / rejected / cancelled |
| frozen_graph | JSONB | 创建时快照的完整图定义（不再运行时 JOIN 模板） |
| initiator_id | UUID | 发起人 |
| created_at | TIMESTAMP | |
| completed_at | TIMESTAMP | |

注意：无 `current_node_ids` 数组列。当前活跃节点通过派生查询获取。

### workflow_tasks

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| instance_id | UUID | FK → instances |
| node_id | VARCHAR | frozen_graph 中的节点 ID |
| assignee_id | UUID | 被分配人 |
| action | VARCHAR | approve / reject / complete / delegate |
| status | VARCHAR | pending / completed / rejected / delegated |
| result | JSONB | 操作结果（评论、意见等） |
| created_at | TIMESTAMP | |
| completed_at | TIMESTAMP | |

### Indexes

```sql
CREATE INDEX idx_workflow_tasks_assignee_status ON workflow_tasks(assignee_id, status);
CREATE INDEX idx_workflow_tasks_instance_status ON workflow_tasks(instance_id, status);
CREATE INDEX idx_workflow_instances_entity ON workflow_instances(entity_type, entity_id);
```

## Condition AST

用 Rust enum 定义条件表达式树，替代原始 JSONB：

```rust
enum Condition {
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
    FieldCompare {
        field: String,
        op: CompareOp,
        value: serde_json::Value,
    },
    EntityStatus { status: String },
    Always,
    Never,
}

enum CompareOp {
    Eq,
    Neq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    In,
}
```

条件在模板创建时验证（字段存在性、类型匹配），不在运行时才发现错误。`ConditionEvaluator` 负责将条件应用于实体上下文进行求值。

## WorkflowEngine Core Logic

### start_instance(template_id, entity_type, entity_id, initiator_id)

1. 加载模板，验证状态为 active
2. 深拷贝 `graph` JSONB 到 `frozen_graph`，记录 `template_version`
3. 创建实例记录
4. 从 `frozen_graph` 找到 start 节点后的第一个节点
5. 解析 assignee 规则，若零候选人则 fail-closed + 报警
6. 为该节点创建 task 并分配审批人/执行人
7. 返回 instance_id

### advance_instance(task_id, action, result)

1. 在事务中执行
2. 标记当前 task 完成/拒绝
3. 从 `frozen_graph`（已加载到实例行上）查找当前节点的出边
4. 用 Condition AST 评估条件，确定下一个节点
5. 如果下一个是 end 节点，标记实例完成
6. 如果是 approval/task 节点，解析 assignee（fail-closed），创建新 task
7. 派发 WorkflowHook::on_approved / on_rejected（如有）

### reject_instance(task_id, reason)

1. 标记 task 为 rejected
2. 根据 node config 中的 reject_action 决定行为：
   - terminate: 直接终止实例
   - back_to_previous: 退回到上一个节点，创建新 task

### get_current_nodes(instance_id)（派生查询）

```sql
SELECT DISTINCT node_id FROM workflow_tasks
WHERE instance_id = $1 AND status = 'pending'
```

替代了 `current_node_ids` 数组，无需维护冗余状态。

## Assignee Configuration (node.config JSONB)

```json
{
  "assignee_type": "role",
  "assignee_value": "manager",
  "multi_approval": "any",
  "reject_action": "terminate",
  "fallback_assignee": "admin"
}
```

- `assignee_type`: `role` | `user` | `department_head` | `initiator_manager`
- `multi_approval`: `any`（任一通过即推进）| `all`（全部通过才推进）
- `fallback_assignee`: **必填**。当 assignee 规则解析为零候选人时，分配给此用户并触发报警

### Fail-closed 策略

1. 解析 assignee 规则，获取候选人列表
2. 若列表为空：使用 `fallback_assignee` 创建 task，同时在 task 的 result 中记录 `"auto_assigned_reason": "no_candidates_found"`
3. 若 `fallback_assignee` 也不存在：暂停实例（status 设为 `suspended`），返回错误

## WorkflowHook Trait（替代 JSONB 回调）

```rust
#[async_trait]
trait WorkflowHook: Send + Sync {
    async fn on_approved(&self, conn: PgExecutor<'_>, instance: &WorkflowInstance, entity_id: Uuid) -> anyhow::Result<()>;
    async fn on_rejected(&self, conn: PgExecutor<'_>, instance: &WorkflowInstance, entity_id: Uuid) -> anyhow::Result<()>;
}

struct WorkflowHookRegistry {
    hooks: HashMap<String, Arc<dyn WorkflowHook>>,
}
```

每个业务实体类型注册一个 hook 实现。模板 config 只标记 `has_callback: true`，引擎根据 `entity_type` 分派到对应的 hook。

## gRPC API

```protobuf
service WorkflowService {
  // Template management
  rpc CreateTemplate(CreateTemplateRequest) returns (Template);
  rpc UpdateTemplate(UpdateTemplateRequest) returns (Template);
  rpc GetTemplate(GetTemplateRequest) returns (Template);
  rpc ListTemplates(ListTemplatesRequest) returns (TemplateList);
  rpc PublishTemplate(PublishTemplateRequest) returns (Template);

  // Instance management
  rpc StartInstance(StartInstanceRequest) returns (Instance);
  rpc GetInstance(GetInstanceRequest) returns (Instance);
  rpc ListInstances(ListInstancesRequest) returns (InstanceList);
  rpc CancelInstance(CancelInstanceRequest) returns (Instance);

  // Task operations
  rpc GetMyTasks(GetMyTasksRequest) returns (TaskList);
  rpc GetTasksByInstance(GetTasksByInstanceRequest) returns (TaskList);
  rpc ApproveTask(TaskActionRequest) returns (Task);
  rpc RejectTask(TaskActionRequest) returns (Task);
  rpc DelegateTask(DelegateTaskRequest) returns (Task);
}
```

### Error Handling

| Status Code | Scenario |
|-------------|----------|
| NOT_FOUND | 模板/实例/任务不存在 |
| FAILED_PRECONDITION | 试图对已完成实例操作、模板 draft 时创建实例 |
| PERMISSION_DENIED | 非审批人试图审批 |
| INVALID_ARGUMENT | 流程定义不合法（缺少 start/end 节点、条件类型不匹配） |
| RESOURCE_EXHAUSTED | assignee 解析为零候选人且无 fallback |

## Implementation Plan

### Step 1: Template CRUD + Condition AST

实现模板 CRUD（graph 作为 JSONB 整体读写）。实现 `Condition` enum 及其验证和求值逻辑。验证在模板创建时执行。

### Step 2: Engine Core + Frozen Graph + Derived State

实现 WorkflowEngine 的 start_instance（含 frozen_graph 快照）和 advance_instance。用派生查询替代 current_node_ids。先用线性流程验证基本通路。

### Step 3: Condition Branches & Parallel

在引擎中加入条件评估和并行网关支持。并行网关的合并通过派生查询检查"该节点所有入边的源 task 是否都已完成"。

### Step 4: WorkflowHook + gRPC API

实现 WorkflowHook trait registry 和各实体的 hook。暴露完整的 gRPC 接口。

## Testing

- Repository 层：sqlx test fixture
- Condition AST：纯 Rust 单元测试，无需数据库
- Engine 层：mock repository 测试引擎逻辑（frozen_graph 从 JSON 反序列化即可）
- 集成测试：完整的"创建模板 → 发起实例 → 审批 → 完成"流程
- 边界测试：驳回后退回、并行节点部分完成、条件分支选择、assignee 解析为零候选人的 fail-closed 行为

## V1 Scope Exclusions

- 拖拽式流程设计器 UI
- 定时器/超时节点
- 子流程嵌套
- 复杂会签规则（V1 只支持 any 和 all）
