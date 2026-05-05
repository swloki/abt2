---
name: Workflow Engine Design
date: 2026-05-05
status: approved
---

# Workflow Engine Design

## Overview

为 ABT 系统引入嵌入式工作流引擎，支持可配置的审批流程和任务分配。引擎完全集成在现有 Rust + PostgreSQL 架构中，零外部依赖。

## Decision: Self-built vs External Library

选择自建引擎，排除 Temporal 等外部工作流平台，原因：

- Temporal Rust SDK 仍为 prerelease，API 不稳定
- Temporal 需要独立的 Temporal Server 部署，与"嵌入式"需求冲突
- ABT 的工作流需求（审批 + 任务分配）复杂度可控，不需要分布式编排能力
- 自建方案保持所有数据在同一个 PostgreSQL 中，事务一致性更易保证

## 核心架构分层

```
┌─────────────────────────────────────────────────┐
│ 静态定义层                                       │
│ 模板管理 + Graph Linter + Condition AST 校验      │
├─────────────────────────────────────────────────┤
│ 动态运行层                                       │
│ 事务驱动的实例流转 + frozen graph 快照 + 派生状态  │
├─────────────────────────────────────────────────┤
│ 自动化调度层                                     │
│ 后台 Worker（超时扫描、提醒、异步 Hook）           │
└─────────────────────────────────────────────────┘
```

## Architecture

遵循现有分层模式：

```
proto/abt/v1/workflow.proto           ← gRPC 定义
abt/src/models/workflow.rs            ← 数据模型（含 Condition AST）
abt/src/repositories/
  workflow_template_repo.rs           ← 模板 CRUD
  workflow_instance_repo.rs           ← 实例 CRUD
  workflow_task_repo.rs               ← 任务 CRUD
  workflow_history_repo.rs            ← 审计历史
abt/src/service/workflow_service.rs   ← Service trait
abt/src/implt/workflow_engine.rs      ← 核心引擎
abt/src/implt/workflow_hooks.rs       ← WorkflowHook trait + 各实体的 hook 实现
abt/src/implt/workflow_worker.rs      ← 后台 Worker（超时扫描、异步 Hook）
abt/src/implt/graph_linter.rs         ← Graph Linter（发布时校验）
abt-grpc/src/handlers/workflow.rs     ← gRPC handler
```

## Design Improvements (from Ideation)

基于 ideation 分析，采纳以下改进：

### Improvement 1: 4 表设计替代 5 表（折叠 nodes + edges 到 JSONB）

将 `workflow_nodes` 和 `workflow_edges` 折叠为 `workflow_templates` 上的一个 JSONB 列 `graph`。新增独立的 `workflow_history` 审计表。V1 没有可视化设计器，流程图很小（3-8 节点），模板是原子发布的。

### Improvement 2: 冻结图快照

实例创建时将完整图定义序列化到 `frozen_graph` JSONB 列。运行时无需 JOIN 模板表，模板编辑不影响运行中的实例。与 BOM 系统的快照模式一致。

### Improvement 3: 移除 current_node_ids 数组

当前活跃节点从 `workflow_tasks WHERE status = 'pending'` 派生查询得出，消除并行网关合并时的行级锁争用。

### Improvement 4: Condition AST 替代原始 JSONB 条件

用 Rust enum 表达式树替代原始 JSONB 条件。条件在模板创建时验证，防止类型不匹配导致的静默路由失败。

### Improvement 5: Fail-closed assignee + 升级路径

assignee 规则解析为零候选人时，fail-closed（暂停流程 + 报警），不跳过审批。每个节点配置必须包含 `fallback_assignee`。

### Improvement 6: workflow_history 决策审计表

独立审计表记录每一次决策（条件判定、超时动作、Hook 执行等），提供完整的可追溯性。

### Improvement 7: 超时与调度 Worker

节点配置支持 `timeout_hours` / `remind_hours_before`，后台 Worker 定期扫描超时任务并执行相应动作。

## Data Model

### workflow_templates

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| entity_type | VARCHAR | 业务实体类型 |
| name | VARCHAR | 模板名称 |
| version | INT | 单调递增版本 |
| status | VARCHAR | draft / active / archived |
| graph | JSONB | 完整流程定义（nodes + edges） |
| graph_checksum | VARCHAR | 图结构哈希 |
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
        "fallback_assignee": "admin",
        "timeout_hours": 48,
        "remind_hours_before": 4
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

### workflow_instances

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| template_id | UUID | 模板引用（仅追溯，运行时不 JOIN） |
| template_version | INT | 创建时版本 |
| entity_type | VARCHAR | 业务实体类型 |
| entity_id | UUID | 业务实体 ID |
| status | VARCHAR | 枚举值见下方说明 |
| frozen_graph | JSONB | 创建时快照的完整图定义 |
| context | JSONB | 运行时变量 + entity_snapshot |
| suspended_reason | JSONB | 挂起原因（如 `{"reason": "no_fallback_assignee", "node_id": "manager_approval"}`） |
| initiator_id | UUID | 发起人 |
| created_at | TIMESTAMP | |
| updated_at | TIMESTAMP | |
| last_advanced_at | TIMESTAMP | 最后推进时间 |
| completed_at | TIMESTAMP | |

**instance status 枚举值：**
- `running` — 流程执行中
- `completed` — 正常完成（到达 end 节点）
- `rejected` — 被驳回且 reject_action = terminate
- `suspended` — 异常挂起（assignee 解析失败、人工暂停等），`suspended_reason` 记录具体原因
- `cancelled` — 发起人主动取消
- `terminated` — 超时终止或其他系统终止

无 `current_node_ids` 数组列。当前活跃节点通过派生查询获取。

### workflow_tasks

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| instance_id | UUID | FK → instances |
| node_id | VARCHAR NOT NULL | frozen_graph 中的节点 ID |
| prev_task_id | UUID | 任务链溯源 |
| assignee_id | UUID | 处理人 |
| status | VARCHAR | 枚举值见下方说明 |
| action | VARCHAR | approve / reject / complete / delegate |
| timeout_action | VARCHAR | 超时动作：auto_approve / auto_reject / escalate / notify（来自节点 config） |
| due_at | TIMESTAMP | 超时截止时间 |
| remind_at | TIMESTAMP | 提醒时间 |
| result | JSONB | 操作结果、意见、auto_reason |
| created_at | TIMESTAMP | |
| completed_at | TIMESTAMP | |

**task status 枚举值：**
- `pending` — 等待处理
- `completed` — 已完成（审批通过 / 任务完成）
- `rejected` — 已拒绝
- `delegated` — 已转交给他人（生成新的 pending task）
- `timed_out` — 超时自动处理（Worker 根据 `timeout_action` 执行）
- `cancelled` — 实例取消导致任务取消

**关于 `prev_task_id`：** 并行场景下一个 task 可能有多个前置 task，V1 保持单父设计（记录触发当前 task 的直接前驱），并行分支各自独立溯源。V2 可按需扩展为 `prev_task_ids JSONB`。

### workflow_history

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| instance_id | UUID | 实例 ID |
| task_id | UUID | 关联任务 |
| node_id | VARCHAR | 节点 ID |
| event_type | VARCHAR | node_enter / edge_triggered / condition_evaluated / task_completed / timeout_action / hook_executed / suspended |
| actor_id | UUID | 操作人（系统用特殊值） |
| payload | JSONB | 详细记录（条件判定结果、超时原因等） |
| created_at | TIMESTAMP | |

### Indexes

```sql
CREATE INDEX idx_workflow_tasks_assignee_status ON workflow_tasks(assignee_id, status, due_at);
CREATE INDEX idx_workflow_tasks_instance_node ON workflow_tasks(instance_id, node_id, status);
CREATE INDEX idx_workflow_instances_entity ON workflow_instances(entity_type, entity_id, status);
CREATE INDEX idx_workflow_history_instance_time ON workflow_history(instance_id, created_at);
```

## Graph Linter（发布时强制校验）

模板从 `draft` 发布为 `active` 时执行以下校验：

1. 必须有且仅有一个 `start` 节点，至少一个 `end` 节点
2. DFS 死循环检测（允许合法汇聚）
3. 每个审批/任务节点必须配置 `fallback_assignee`
4. Condition AST 字段白名单 + 类型校验
5. 并行 Join 节点的 `incoming_edges` 配置必须与实际入边数量匹配
6. 计算 `graph_checksum`（SHA-256 of `graph -> 'nodes' || graph -> 'edges'`）用于完整性校验

校验失败则拒绝发布，返回具体错误信息。

## Condition AST 与求值

```rust
pub enum Condition {
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

pub enum CompareOp {
    Eq, Neq, Gt, GtEq, Lt, LtEq, In,
}

pub struct EvaluationContext {
    pub entity_snapshot: serde_json::Value,
    pub variables: HashMap<String, serde_json::Value>,
    pub initiator: UserInfo,
}
```

条件在模板创建时验证（字段存在性、类型匹配），运行时通过 `EvaluationContext` 求值。`entity_snapshot` 在实例创建时写入 `context` JSONB，条件评估完全自包含，不依赖外部状态。

## WorkflowEngine Core Logic

### start_instance(template_id, entity_type, entity_id, initiator_id)

1. 加载模板，验证状态为 active
2. 深拷贝 `graph` JSONB 到 `frozen_graph`，记录 `template_version`
3. 写入 `entity_snapshot` 到 `context` JSONB
4. 创建实例记录
5. 从 `frozen_graph` 找到 start 节点后的第一个节点
6. 解析 assignee 规则（fail-closed），计算 `due_at` / `remind_at`
7. 为该节点创建 task
8. 记录 `workflow_history`（`node_enter` 事件）
9. 返回 instance_id

### advance_instance(task_id, action, result)

```rust
pub async fn advance_instance(task_id: Uuid, action: Action, result: Value) -> Result<()> {
    let mut tx = pool.begin().await?;

    // 1. 锁实例
    let instance = lock_instance_for_update(&mut tx, instance_id).await?;

    // 2. 更新当前任务
    update_task(&mut tx, task_id, action, result).await?;

    // 3. 评估下一节点（Condition AST）
    let context = EvaluationContext::from_instance(&instance);
    let next_nodes = engine.evaluate_next(&instance.frozen_graph, &action, &context)?;

    // 4. 创建新任务（含 due_at / remind_at）
    for node in next_nodes {
        create_task(&mut tx, &instance, node).await?;
    }

    // 5. 记录审计历史
    record_history(&mut tx, event_details).await?;

    tx.commit().await?;

    // 6. 事务外异步执行 Hook
    hooks.dispatch_async(&instance, next_nodes).await;

    Ok(())
}
```

### get_current_nodes(instance_id)（派生查询）

```sql
SELECT DISTINCT node_id FROM workflow_tasks
WHERE instance_id = $1 AND status = 'pending'
```

### 并行网关 Join 判断

从 `frozen_graph` 中获取目标节点的所有入边，通过派生查询检查所有入边的源 task 是否都已完成。

## Assignee Configuration (node.config JSONB)

```json
{
  "assignee_type": "role",
  "assignee_value": "manager",
  "multi_approval": "any",
  "reject_action": "terminate",
  "fallback_assignee": "admin",
  "timeout_hours": 48,
  "remind_hours_before": 4,
  "timeout_action": "escalate"
}
```

- `assignee_type`: `role` | `user` | `department_head` | `initiator_manager`
- `multi_approval`: `any`（任一通过即推进）| `all`（全部通过才推进）
- `reject_action`: `terminate`（终止实例）| `back_to_previous`（退回上一节点）
  - `back_to_previous` 实现：查找 `prev_task_id` 对应的节点，在该节点创建新 task，保留原 task 的 result 作为上下文
- `fallback_assignee`: **必填**（Graph Linter 强制校验）
- `timeout_hours`: 可选，超时截止时间
- `remind_hours_before`: 可选，提前提醒时间
- `timeout_action`: 可选，超时动作（`auto_approve` / `auto_reject` / `escalate` / `notify`）

### Fail-closed 策略

1. 解析 assignee 规则，获取候选人列表
2. 若列表为空：使用 `fallback_assignee` 创建 task，同时在 result 中记录 `"auto_assigned_reason": "no_candidates_found"`
3. 若 `fallback_assignee` 也不存在：暂停实例（status 设为 `suspended`，写入 `suspended_reason`），返回 `WORKFLOW_SUSPENDED` 错误

## WorkflowHook Trait（替代 JSONB 回调）

```rust
#[async_trait]
trait WorkflowHook: Send + Sync {
    async fn on_approved(&self, instance: &WorkflowInstance, entity_id: Uuid) -> anyhow::Result<()>;
    async fn on_rejected(&self, instance: &WorkflowInstance, entity_id: Uuid) -> anyhow::Result<()>;
}

struct WorkflowHookRegistry {
    hooks: HashMap<String, Arc<dyn WorkflowHook>>,
}
```

Hook 在事务提交后异步执行（由 Worker 处理），避免 hook 失败导致状态回滚。每个业务实体类型注册一个 hook 实现。

## 超时与调度 Worker

- 节点 config 支持 `timeout_hours`、`remind_hours_before`、`timeout_action`
- `timeout_action` 值：`auto_approve` / `auto_reject` / `escalate` / `notify`
- Worker 扫描频率：默认 5 分钟，可通过 Admin API 动态调整
- 使用 `FOR UPDATE SKIP LOCKED` 防重复处理
- 超时动作记录到 `workflow_history`（`timeout_action` 事件）
- Worker 根据 `timeout_action` 字段执行对应动作并更新 task status 为 `timed_out`

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
| INVALID_ARGUMENT | 流程定义不合法（Graph Linter 校验失败、条件类型不匹配） |
| WORKFLOW_SUSPENDED | assignee 解析为零候选人且无 fallback，实例已挂起 |

## Implementation Plan

### Step 1: Template CRUD + Graph Linter + Condition AST

实现模板 CRUD（graph 作为 JSONB 整体读写）。实现 Graph Linter（发布时校验）。实现 `Condition` enum 及其验证和求值逻辑。

### Step 2: Engine Core + Frozen Graph + Derived State

实现 WorkflowEngine 的 start_instance（含 frozen_graph 快照 + entity_snapshot）和 advance_instance。用派生查询替代 current_node_ids。先用线性流程验证基本通路。

### Step 3: Parallel Gateway + Timeout Worker

实现并行网关的 join 判断（入边 + 派生查询）。实现后台 Worker（超时扫描、提醒，`FOR UPDATE SKIP LOCKED`）。实现 `workflow_history` 审计记录。

### Step 4: Async Hook + Complete gRPC API

实现 WorkflowHook trait registry 和各实体的 hook（事务外异步执行）。暴露完整的 gRPC 接口。

## Testing

- Repository 层：sqlx test fixture
- Graph Linter：纯 Rust 单元测试（cycle detection、missing nodes、invalid conditions）
- Condition AST：纯 Rust 单元测试，无需数据库
- Engine 层：mock repository 测试引擎逻辑（frozen_graph 从 JSON 反序列化）
- Worker：集成测试超时扫描、`SKIP LOCKED` 防重复
- 端到端：完整的"创建模板 → 发布 → 发起实例 → 审批/超时 → 完成"流程
- 边界测试：驳回后退回、并行节点部分完成、条件分支选择、assignee 解析为零候选人的 fail-closed、fallback 不存在的 suspended 行为

## V1 Scope Exclusions

- 拖拽式流程设计器 UI
- 子流程嵌套
- 复杂会签规则（V1 只支持 any 和 all）
