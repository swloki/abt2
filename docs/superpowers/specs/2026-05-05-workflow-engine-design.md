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
        "remind_hours_before": 4,
        "timeout_action": "escalate"
      }
    },
    {
      "id": "dept_approval",
      "node_type": "approval",
      "name": "部门审批",
      "config": {
        "assignee_type": "role",
        "assignee_value": "dept_head",
        "multi_approval": "any",
        "reject_action": "back_to_previous",
        "fallback_assignee": "admin"
      }
    },
    {
      "id": "merge_point",
      "node_type": "join",
      "name": "汇聚",
      "config": {
        "join_strategy": "all"
      }
    },
    {
      "id": "end",
      "node_type": "end",
      "name": "结束"
    }
  ],
  "edges": [
    {"from": "start", "to": "manager_approval"},
    {"from": "start", "to": "dept_approval"},
    {"from": "manager_approval", "to": "merge_point"},
    {"from": "dept_approval", "to": "merge_point"},
    {"from": "merge_point", "to": "end"}
  ]
}
```

**Join 节点说明：** `merge_point` 有两条入边（来自 `manager_approval` 和 `dept_approval`），`join_strategy: "all"` 表示所有入边的源 task 都完成后才激活。Graph Linter 校验时验证入边数量与实际 edges 匹配。

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
- `rejected` — 实例因驳回终止（reject_action = terminate）
- `suspended` — 异常挂起（assignee 解析失败、人工暂停等），`suspended_reason` 记录具体原因
- `cancelled` — 发起人主动取消
- `terminated` — 系统终止（超时 auto_reject 导致终止，与 `rejected` 区分：`rejected` 是人工驳回，`terminated` 是系统行为）

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
| event_type | VARCHAR | node_enter / edge_triggered / condition_evaluated / task_completed / timeout_action / hook_executed / back_to_previous / multi_approval_waiting / suspended |
| actor_id | UUID | 操作人（`SYSTEM_USER = 00000000-0000-0000-0000-000000000000` 表示系统/Worker 操作） |
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
4. Condition AST 字段白名单 + 类型校验（创建时初步校验 + 发布时最终确认，两阶段使用同一份白名单配置）
5. 发布时最终确认：检查所有 edge 的 condition 中引用的 field 是否在 `entity_type` 对应的白名单中
6. 并行 Join 节点的 `incoming_edges` 配置必须与实际入边数量匹配
7. `reject_action: back_to_previous` 只能配置在入边源头为人工节点（`approval` / `task`）的节点上，防止退回到 `start` 或 `join` 节点导致逻辑死循环
8. 计算 `graph_checksum`（SHA-256，对规范化 JSON 字符串 `canonicalize(graph.nodes) ++ canonicalize(graph.edges)` 计算哈希）用于完整性校验

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
5. 从 `frozen_graph` 找到 start 节点的所有出边对应的目标节点（支持并行启动：start 可连接多个后续节点）
6. 对每个目标节点，解析 assignee 规则（fail-closed），计算 `due_at` / `remind_at`
7. 为每个目标节点创建 task
8. 记录 `workflow_history`（`node_enter` 事件）
9. 返回 instance_id

### advance_instance(task_id, action, result)

```rust
pub async fn advance_instance(task_id: Uuid, action: Action, result: Value) -> Result<()> {
    let mut tx = pool.begin().await?;

    // 1. 查询 task 获取 instance_id
    let task = find_task_for_update(&mut tx, task_id).await?;

    // 2. 按 instance ID 顺序锁定实例（防止并行审批死锁）
    //    并行网关中两个审批人同时操作时，都先锁实例再处理任务，
    //    确保在实例级别排队，Join 判定基于最准确的快照
    let instance = lock_instance_for_update(&mut tx, task.instance_id).await?;

    // 2. 更新当前任务
    update_task(&mut tx, task_id, action, result).await?;

    // 3. 检查 multi_approval 语义
    //    若当前节点的 config.multi_approval == "all"，
    //    且该 node_id 下仍有 pending task，则不推进（等待全部完成）
    //    但若 action == reject，直接触发 reject_action，不再等待其他人
    let node_config = instance.frozen_graph.get_node_config(&task.node_id)?;
    if action == Action::Reject {
        // 任一 reject 即视为节点 reject（multi_approval=all 场景）
        return handle_reject(&mut tx, &instance, &task, &node_config).await;
    }
    if node_config.multi_approval == MultiApproval::All {
        let remaining = count_pending_tasks(&mut tx, instance.id, &task.node_id).await?;
        if remaining > 0 {
            // 记录等待事件，便于监控
            record_history(&mut tx, HistoryEvent::multi_approval_waiting(
                instance.id, task.id, task.node_id, remaining
            )).await?;
            tx.commit().await?;
            return Ok(()); // 还有人没审批，不推进
        }
    }

    // 4. 评估下一节点（Condition AST）
    let context = EvaluationContext::from_instance(&instance);
    let next_nodes = engine.evaluate_next(&instance.frozen_graph, &task.node_id, &action, &context)?;

    // 5. 过滤出真正可激活的节点（Join 检查）
    //    并行汇聚：获取目标节点所有入边的源节点，
    //    检查这些源节点的 task 是否都已完成
    let mut ready_nodes = vec![];
    for node in next_nodes {
        let incoming = instance.frozen_graph.get_incoming_edges(&node.id)?;
        if incoming.len() > 1 {
            let all_done = check_join_ready(&mut tx, instance.id, &incoming).await?;
            if !all_done {
                continue; // 还有并行分支未完成，不激活此节点
            }
        }
        ready_nodes.push(node);
    }

    // 6. 创建新任务（含 due_at / remind_at / timeout_action）
    for node in &ready_nodes {
        create_task(&mut tx, &instance, node, Some(task_id)).await?;
    }

    // 7. 记录审计历史
    record_history(&mut tx, event_details).await?;

    tx.commit().await?;

    // 8. 事务外异步执行 Hook
    hooks.dispatch_async(&instance, &ready_nodes).await;

    Ok(())
}
```

### get_current_nodes(instance_id)（派生查询）

```sql
SELECT DISTINCT node_id FROM workflow_tasks
WHERE instance_id = $1 AND status = 'pending'
```

### 并行网关 Join 判断

从 `frozen_graph` 中获取目标节点的所有入边，通过派生查询检查所有入边的源 task 是否都已完成。具体逻辑见 `advance_instance` 伪代码步骤 5。

### Multi-approval = "all" 处理

当一个 `approval` 节点的 `multi_approval = "all"` 时，该节点会创建多个并行 task（每个审批人一个）。任一 task 完成时不立即推进，而是检查同一 `node_id` 下是否还有 `pending` task。只有当该节点所有 task 都完成后才评估出边并推进。

**multi_approval = "all" + reject：** 任一审批人 reject 即视为整个节点 reject，立即按 `reject_action` 执行，不再等待其他审批人。同一节点下其余 `pending` task 标记为 `cancelled`。

**multi_approval = "all" + timeout：** 若其中一人未操作且超时，Worker 根据 `timeout_action` 处理该 task。若 `timeout_action = auto_reject`，等同于该人 reject，触发上述 reject 逻辑，同一节点下其余 pending task 标记为 `cancelled`。

### Reject 处理（reject_action 详解）

**`terminate`：** 标记当前 task 为 rejected，实例 status 设为 `rejected`。

**`back_to_previous`：**
1. 标记当前 task 为 rejected，记录 `workflow_history`（`back_to_previous` 事件）
2. 沿 `frozen_graph` 反向查找当前节点的入边，确定前驱节点
3. 若有多个入边（复杂分支），V1 取最近一条入边对应的源节点（按 edge 顺序取第一条）
4. 在前驱节点创建新 task，`prev_task_id` 指向当前 rejected task
5. 新 task 的 `result` 包含 `{"rejection_context": {"original_task_id": "...", "reason": "..."}}`
6. V1 只支持退回到直接前驱节点，不支持指定退回目标或跳级退回

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
- `reject_action`: `terminate`（终止实例）| `back_to_previous`（退回上一节点，沿 frozen_graph 入边反向查找，V1 仅支持直接前驱）
- `fallback_assignee`: **必填**（Graph Linter 强制校验）
- `timeout_hours`: 可选，超时截止时间
- `remind_hours_before`: 可选，提前提醒时间
- `timeout_action`: 可选，超时动作（`auto_approve` / `auto_reject` / `escalate` / `notify`）

### Fail-closed 策略

1. 解析 assignee 规则，获取候选人列表
2. 若列表为空：使用 `fallback_assignee` 创建 task，同时在 result 中记录 `"auto_assigned_reason": "no_candidates_found"`
3. 若 `fallback_assignee` 也不存在：暂停实例（status 设为 `suspended`，写入 `suspended_reason`），返回 gRPC `ABORTED` 错误，response detail 携带 `suspended_reason`

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
- Worker 扫描频率：默认 1 分钟（默认值存于环境变量 `WORKER_SCAN_INTERVAL_SECS`），可通过 Admin API 动态调整。优先级：Admin 配置 > 环境变量。1 分钟而非 5 分钟的理由：`remind_at` 提醒功能对实时性敏感，5 分钟间隔可能导致用户在过期后才收到"即将过期"提醒；`due_at` 索引 + `SKIP LOCKED` 使 1 分钟扫描的 PostgreSQL 负载极低
- 使用 `FOR UPDATE SKIP LOCKED` 防重复处理
- 超时动作记录到 `workflow_history`（`timeout_action` 事件）
- Worker 根据 `timeout_action` 字段执行对应动作并更新 task status 为 `timed_out`
- Worker 操作中 `actor_id` 使用 `SYSTEM_USER`（`00000000-0000-0000-0000-000000000000`），与真人操作明确区分
- `assignee_id` 同样使用 `SYSTEM_USER` 标识系统自动分配的任务（如 `fallback_assignee` 触发时）

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
| ABORTED | 实例已挂起（assignee 解析为零候选人且无 fallback），response 携带 `suspended_reason` 供前端展示 |

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
- **重点覆盖组合场景（并行 + multi_approval=all）：**
  - 甲审批通过，乙未操作超时 → auto_reject → 甲的 task cancelled → 实例 reject/terminate
  - 甲审批通过，乙 reject → 立即触发 reject_action → 甲的 task cancelled
  - 甲审批通过，乙审批通过 → 正常推进到 join 节点
  - back_to_previous 退回到 start 或 join 节点时被 Graph Linter 拦截

## V1 Scope Exclusions

- 拖拽式流程设计器 UI
- 子流程嵌套
- 复杂会签规则（V1 只支持 any 和 all）
