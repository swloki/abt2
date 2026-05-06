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
abt/src/implt/workflow_actions.rs     ← ActionRegistry + 各业务 action 实现
abt/src/implt/workflow_hooks.rs       ← WorkflowHook trait + 各实体的 hook 实现
abt/src/implt/workflow_worker.rs      ← 后台 Worker（超时扫描、异步 Hook）
abt/src/implt/graph_linter.rs         ← Graph Linter（发布时校验）
abt-grpc/src/handlers/workflow.rs     ← gRPC handler
```

## Design Improvements (from Ideation)

基于 ideation 分析，采纳以下改进：

### Improvement 0: auto_task 节点类型（系统自动执行的业务动作）

除人工审批节点外，引入 `auto_task` 节点类型。引擎推进到 auto_task 时，通过 `ActionRegistry` 查找并执行注册的业务动作（如生成生产定单、创建领料单），执行成功后自动推进到下一节点。**每个 auto_task 独立事务**：成功即提交并记录 history，失败则仅回滚当前节点，实例 suspended。链式 auto_task 逐个独立执行，失败后重试从断点继续。运维可通过 Admin API 手动重试。

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

**模板生命周期约束：** 模板一旦发布为 `active` 即进入不可变状态，`graph`、`graph_checksum`、`entity_type` 均不可修改。需要变更只能通过 `CreateTemplate`（克隆当前模板 → 修改 → 发布为新版本）。`UpdateTemplate` 仅允许对 `draft` 状态的模板操作。`active` 模板只能转换为 `archived`，不可回退。

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
      "id": "gen_prod_order",
      "node_type": "auto_task",
      "name": "生成生产定单",
      "config": {
        "action": "create_production_order",
        "retryable": true
      }
    },
    {
      "id": "gen_schedule",
      "node_type": "auto_task",
      "name": "生成提排单",
      "config": {
        "action": "create_production_schedule",
        "retryable": true
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
    {"from": "merge_point", "to": "gen_prod_order"},
    {"from": "gen_prod_order", "to": "gen_schedule"},
    {"from": "gen_schedule", "to": "end"}
  ]
}
```

**Join 节点说明：** `merge_point` 有两条入边（来自 `manager_approval` 和 `dept_approval`），`join_strategy: "all"` 表示所有入边的源 task 都完成后才激活。Graph Linter 校验时验证入边数量与实际 edges 匹配。`incoming_edges` 可在运行时从 `frozen_graph.edges` 反向计算得出，也可在实例创建时预存储到 `context.join_progress` 中，示例中保持简洁不做预存储。

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
| context | JSONB | 运行时变量 + entity_snapshot + join_progress |
| suspended_reason | JSONB | 挂起原因（如 `{"reason": "no_fallback_assignee", "node_id": "manager_approval"}`） |
| initiator_id | UUID | 发起人 |
| created_at | TIMESTAMP | |
| updated_at | TIMESTAMP | |
| last_advanced_at | TIMESTAMP | 最后推进时间 |
| completed_at | TIMESTAMP | |

**instance status 枚举值：**
- `running` — 流程执行中
- `completed` — 正常完成（到达 end 节点）
- `rejected` — 实例因人工驳回（reject_action = terminate）而终止
- `suspended` — 异常挂起（assignee 解析失败、人工暂停等），`suspended_reason` 记录具体原因
- `cancelled` — 发起人主动取消
- `terminated` — 系统终止（超时 auto_reject 导致终止，与 `rejected` 区分：`rejected` 是人工驳回，`terminated` 是系统行为）

无 `current_node_ids` 数组列。当前活跃节点通过派生查询获取。

**context JSONB 结构：**
```json
{
  "entity_snapshot": { ... },
  "variables": { "key": "value" },
  "join_progress": {
    "merge_point": ["manager_approval", "dept_approval"]
  }
}
```
`join_progress` 记录每个 join 节点已完成的入边源节点列表。Join 判定优先使用内存中的 `join_progress`，避免反复扫描 `workflow_tasks` 表。每次 task 完成时更新对应 join 节点的 progress。

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
CREATE INDEX idx_workflow_tasks_pending_due ON workflow_tasks(status, due_at) WHERE status = 'pending';
CREATE INDEX idx_workflow_instances_entity ON workflow_instances(entity_type, entity_id, status);
CREATE INDEX idx_workflow_history_instance_time ON workflow_history(instance_id, created_at);
```

## Graph Linter（发布时强制校验）

模板从 `draft` 发布为 `active` 时执行以下校验：

1. 必须有且仅有一个 `start` 节点，至少一个 `end` 节点
2. DFS 死循环检测（允许合法汇聚）
3. 每个 approval 节点必须配置 `fallback_assignee`
4. 每个 auto_task 节点必须配置 `action`，且 `action` 值必须在当前已注册的 ActionRegistry 中存在（发布时校验，未注册的 action 拒绝发布）
5. Condition AST 字段白名单 + 类型校验（创建时初步校验 + 发布时最终确认，两阶段使用同一份白名单配置）
6. 发布时最终确认：检查所有 edge 的 condition 中引用的 field 是否在 `entity_type` 对应的白名单中
7. 并行 Join 节点的 `incoming_edges` 配置必须与实际入边数量匹配
8. `reject_action: back_to_previous` 只能配置在入边源节点全部为人工节点（`approval`）的节点上，防止退回到 `start`、`join` 或 `auto_task` 节点导致逻辑死循环。此外，配置 `back_to_previous` 的节点在 frozen_graph 中必须只有唯一入边（即唯一前驱），若存在多条入边且无法唯一确定退回目标，Linter 应拒绝发布
9. 计算 `graph_checksum`（SHA-256，对规范化 JSON 字符串 `canonicalize(graph.nodes) ++ canonicalize(graph.edges)` 计算哈希）用于完整性校验

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

impl EvaluationContext {
    /// 查找变量，支持未来扩展到外部数据源（如查询其他服务、数据库）
    /// V1 从 variables map 中查找，V2 可重写此方法接入外部系统
    pub fn get_external_value(&self, key: &str) -> Option<serde_json::Value> {
        self.variables.get(key).cloned()
    }
}
```

条件在模板创建时验证（字段存在性、类型匹配），运行时通过 `EvaluationContext` 求值。`entity_snapshot` 在实例创建时写入 `context` JSONB，条件评估完全自包含，不依赖外部状态。

### EntitySnapshotProvider

引擎本身不知道如何从不同业务实体（product/bom/order）中提取快照数据。通过 trait 向业务层开口：

```rust
#[async_trait]
trait EntitySnapshotProvider: Send + Sync {
    async fn get_snapshot(&self, entity_type: &str, entity_id: Uuid) -> Result<Value>;
}

struct EntitySnapshotRegistry {
    providers: HashMap<String, Arc<dyn EntitySnapshotProvider>>,
}
```

每个业务模块注册自己的 `EntitySnapshotProvider` 实现。`start_instance` 时引擎通过注册表获取快照，保持引擎不耦合具体实体。

### ActionRegistry（auto_task 的业务动作注册表）

```rust
#[async_trait]
trait AutoAction: Send + Sync {
    /// 在独立事务内执行业务动作。接收 instance 上下文，可读取 entity_snapshot。
    /// 执行结果通过返回值传递，引擎写入 workflow_history。
    ///
    /// ## 约束（开发者必须遵守）
    /// 1. 纯数据库操作：禁止调用外部 REST/gRPC 服务、发送邮件/消息等。
    ///    外部副作用必须放在 WorkflowHook（事务后异步执行）中。
    /// 2. 幂等性：使用 instance_id + node_id 作为幂等键，重试时必须安全。
    ///    推荐：INSERT 前先检查是否已存在（WHERE instance_id = $1 AND node_id = $2）。
    /// 3. 快速完成：单个 action 执行时间不应超过 5 秒。
    ///    引擎会设置 SET LOCAL statement_timeout 防止长事务。
    async fn execute(&self, tx: &mut Transaction, instance: &WorkflowInstance) -> Result<ActionOutput>;
}

struct ActionOutput {
    /// 可选：写入 context.variables 的更新（供后续条件评估使用）
    pub variables_update: HashMap<String, serde_json::Value>,
}

struct ActionRegistry {
    actions: HashMap<String, Arc<dyn AutoAction>>,
}

impl ActionRegistry {
    fn register(&mut self, name: &str, action: Arc<dyn AutoAction>) {
        self.actions.insert(name.to_string(), action);
    }

    fn get(&self, name: &str) -> Option<&Arc<dyn AutoAction>> {
        self.actions.get(name)
    }

    /// Graph Linter 发布校验时调用：检查 action 是否已注册
    fn is_registered(&self, name: &str) -> bool {
        self.actions.contains_key(name)
    }
}
```

各业务模块在初始化时注册 action：

```rust
registry.register("create_production_order", Arc::new(CreateProductionOrderAction));
registry.register("create_production_schedule", Arc::new(CreateProductionScheduleAction));
registry.register("create_material_requisition", Arc::new(CreateMaterialRequisitionAction));
```

Action 接收 `&mut Transaction`，可安全进行数据库写操作，与引擎状态更新在同一事务内。Action 执行失败时事务整体回滚，引擎将实例标记为 `suspended`。

## WorkflowEngine Core Logic

### start_instance(entity_type, entity_id, initiator_id)

1. 按 `entity_type` 查找 active 模板（`SELECT * FROM workflow_templates WHERE entity_type = $1 AND status = 'active' LIMIT 1`），验证状态为 active
2. 深拷贝 `graph` JSONB 到 `frozen_graph`，记录 `template_version`
3. 写入 `entity_snapshot` 到 `context` JSONB，初始化 `join_progress`（遍历 frozen_graph 中所有 `node_type == "join"` 的节点，每个节点映射一个空数组）
4. 创建实例记录
5. 从 `frozen_graph` 找到 start 节点的所有出边对应的目标节点（支持并行启动：start 可连接多个后续节点）
6. 对每个目标节点按类型处理：
   - `approval`：解析 assignee 规则（fail-closed），计算 `due_at` / `remind_at`，创建 task
   - `auto_task`：在当前事务内执行注册的 action，成功后自动推进到下一节点（见 auto_task 执行规则）
   - `join`/`end`：由引擎按正常流程处理
7. 记录 `workflow_history`（`node_enter` 事件）
8. 返回 instance_id

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

    // 2b. 立即更新 join_progress：将当前 task 的 node_id 追加到所有
    //     以此 node 为入边源的 join 节点的 progress 列表中
    //     （在 Join 检查之前更新，确保后续判定基于最新状态）
    update_join_progress(&mut tx, &instance, &task.node_id).await?;

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
    //    优先使用 context.join_progress 判断：检查 join_progress[node_id] 是否包含所有入边源
    //    若 join_progress 不可用则 fallback 到 tasks 表派生查询
    let mut ready_nodes = vec![];
    for node in next_nodes {
        let incoming = instance.frozen_graph.get_incoming_edges(&node.id)?;
        if incoming.len() > 1 {
            // 优先从 context.join_progress 检查（已在步骤 2b 中更新）
            let all_done = check_join_progress(&instance.context, &node.id, &incoming)
                .unwrap_or_else(|| check_join_ready(&mut tx, instance.id, &incoming));
            if !all_done {
                continue; // 还有并行分支未完成，不激活此节点
            }
        }
        ready_nodes.push(node);
    }

    // 6. 按节点类型处理每个就绪节点
    for node in &ready_nodes {
        match node.node_type {
            NodeType::Approval => {
                create_task(&mut tx, &instance, node, Some(task_id)).await?;
            }
            NodeType::AutoTask => {
                // 独立事务执行：先提交当前事务，再开新事务
                tx.commit().await?;

                let result = engine.execute_auto_task(&instance, node).await;
                match result {
                    Ok(output) => {
                        // 成功：记录 history，继续推进到下一个节点
                        // execute_auto_task 内部已 commit 并记录 history
                        // 引擎继续评估 node 的出边，处理下一个节点
                        engine.advance_from_node(&instance, node).await?;
                        return Ok(());
                    }
                    Err(e) => {
                        // 失败：suspended 实例（独立事务），返回错误
                        engine.suspend_instance(&instance, node, &e).await?;
                        return Err(e);
                    }
                }
            }
            NodeType::End => {
                update_instance_status(&mut tx, instance.id, "completed").await?;
                record_history(&mut tx, HistoryEvent::node_enter(instance.id, node.id)).await?;
                hooks.dispatch_async(&instance, &[node]).await;
            }
            NodeType::Join | NodeType::Start => {
                // 这些类型由引擎框架处理，不在此分支创建 task
            }
        }
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

**multi_approval = "all" + reject：** 任一审批人 reject 即视为整个节点 reject，立即按 `reject_action` 执行，不再等待其他审批人。同一节点下其余 `pending` task 标记为 `cancelled`，result 中写入 `{"cancelled_reason": "peer_rejected"}`。

**multi_approval = "all" + timeout：** 若其中一人未操作且超时，Worker 根据 `timeout_action` 处理该 task。若 `timeout_action = auto_reject`，等同于该人 reject，触发上述 reject 逻辑，同一节点下其余 pending task 标记为 `cancelled`。

### Reject 处理（reject_action 详解）

**`terminate`：** 标记当前 task 为 rejected，实例 status 设为 `rejected`。

**`back_to_previous`：**
1. 标记当前 task 为 rejected，记录 `workflow_history`（`back_to_previous` 事件）
2. 沿 `frozen_graph` 反向查找当前节点的入边，确定前驱节点
3. 若有多个入边（复杂分支），V1 取最近一条入边对应的源节点（按 edge 顺序取第一条）
4. 在前驱节点创建新 task，`prev_task_id` 指向当前 rejected task
5. 新 task 的 `result` 包含 `{"rejection_context": {"original_task_id": "...", "reason": "..."}}`
6. V1 只支持退回到直接前驱节点，不支持指定退回目标或跳级退回。Graph Linter 强制校验：配置 `back_to_previous` 的节点必须只有唯一入边，多入边节点不允许配置此 reject_action

### auto_task 执行规则

**正常流程（每个 auto_task 独立事务）：**
1. 引擎推进到 auto_task 节点时，开启新事务
2. 在事务内 `SET LOCAL statement_timeout`（默认 5 秒，可在节点 config 中配置 `timeout_seconds`）
3. 从 `ActionRegistry` 查找 `config.action` 对应的 handler
4. 调用 `handler.execute(&mut tx, &instance)`
5. 执行成功：COMMIT 事务，将 `ActionOutput.variables_update` 合并到 `instance.context.variables`，记录 `workflow_history`（`event_type = auto_task_executed`）
6. 自动评估出边，推进到下一节点
7. 若下一节点也是 auto_task，开新事务重复上述步骤

**失败处理：**
1. auto_task 执行失败时，当前事务 ROLLBACK（仅回滚此节点，前面已提交的不受影响）
2. 引擎在独立事务中将实例状态设为 `suspended`，写入 `suspended_reason`：
   ```json
   {"reason": "auto_task_failed", "node_id": "gen_prod_order", "action": "create_production_order", "error": "..."}
   ```
3. 记录 `workflow_history`（`event_type = auto_task_executed, payload.success = false`）
4. 返回 gRPC `ABORTED` 错误
5. 运维通过 `RetryAutoTask` Admin API 手动重试（见下方）

### RetryAutoTask 安全机制

`RetryAutoTask` Admin API 必须满足以下前置条件：
1. 实例状态必须为 `suspended`（`FOR UPDATE` 锁定，防止并发重试）
2. `suspended_reason.reason` 必须为 `auto_task_failed`
3. 重试的 `node_id` 必须与 `suspended_reason.node_id` 匹配
4. 支持可选参数 `refresh_snapshot: bool`：
   - `true`：重试前通过 EntitySnapshotProvider 重新获取业务实体快照，更新 `context.entity_snapshot`
   - `false`（默认）：使用原快照
   - 建议：若实例 suspended 时间超过 1 小时，运维应选择 `refresh_snapshot = true`
5. 重试时 action 实现的幂等性保证：即使前一次执行实际已成功（但客户端超时），重试不会重复创建资源

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

**Hook 失败处理：**
- 失败时写入 `workflow_history`（`event_type = hook_executed`，payload 含 `{"success": false, "error": "..."}`）
- V1 不做自动重试，但提供 `RetryFailedHook` Admin API 入口，运维可手动触发重试，防止 hook 失败后状态永久不一致

## 超时与调度 Worker

- 节点 config 支持 `timeout_hours`、`remind_hours_before`、`timeout_action`
- `timeout_action` 值：`auto_approve` / `auto_reject` / `escalate` / `notify`
- Worker 扫描频率：默认 1 分钟（默认值存于环境变量 `WORKER_SCAN_INTERVAL_SECS`），可通过 Admin API 动态调整。优先级：Admin 配置 > 环境变量。1 分钟而非 5 分钟的理由：`remind_at` 提醒功能对实时性敏感，5 分钟间隔可能导致用户在过期后才收到"即将过期"提醒；`due_at` 索引 + `SKIP LOCKED` 使 1 分钟扫描的 PostgreSQL 负载极低
- 使用 `FOR UPDATE SKIP LOCKED` 防重复处理
- 超时动作记录到 `workflow_history`（`timeout_action` 事件）
- Worker 根据 `timeout_action` 字段执行对应动作并更新 task status 为 `timed_out`
- Worker 操作中 `actor_id` 使用 `SYSTEM_USER`（`00000000-0000-0000-0000-000000000000`），与真人操作明确区分
- 系统自动分配的任务（如 `fallback_assignee` 触发时）`assignee_id` 存放实际被分配的用户，`workflow_history` 中对应事件的 `actor_id` 为 `SYSTEM_USER`
- Worker 使用 `tokio_util::sync::CancellationToken` 实现优雅关闭：收到取消信号后，Worker 等待当前 DB 事务完成再退出，避免中途终止导致数据不一致。`CancellationToken` 由 `abt-grpc` server 的 shutdown 信号触发
- **扩展性说明：** 日活实例超过 5000 时，建议将扫描间隔调整为 2-3 分钟，或按 `entity_type` 分桶扫描以降低单次扫描压力

## gRPC API

```protobuf
service WorkflowService {
  // Template management
  rpc CreateTemplate(CreateTemplateRequest) returns (Template);
  rpc CreateTemplateVersion(CreateTemplateVersionRequest) returns (Template); // 克隆 active 模板为新 draft
  rpc UpdateTemplate(UpdateTemplateRequest) returns (Template); // 仅 draft 状态
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

  // Admin
  rpc RetryFailedHook(RetryFailedHookRequest) returns (RetryFailedHookResponse);
  rpc RetryAutoTask(RetryAutoTaskRequest) returns (RetryAutoTaskResponse);
}
```

### Error Handling

| Status Code | Scenario |
|-------------|----------|
| NOT_FOUND | 模板/实例/任务不存在 |
| FAILED_PRECONDITION | 试图对已完成实例操作、模板 draft 时创建实例 |
| PERMISSION_DENIED | 非审批人试图审批 |
| INVALID_ARGUMENT | 流程定义不合法（Graph Linter 校验失败、条件类型不匹配） |
| ABORTED | 实例已挂起（assignee 解析为零候选人且无 fallback，或 auto_task 执行失败），response 携带 `suspended_reason` 供前端展示 |
| INTERNAL | auto_task action 未注册（引擎内部错误，不应发生在生产环境） |

## Implementation Plan

### Step 1: Template CRUD + Graph Linter + Condition AST

实现模板 CRUD（graph 作为 JSONB 整体读写）。实现 Graph Linter（发布时校验，含 auto_task action 注册校验）。实现 `Condition` enum 及其验证和求值逻辑。

### Step 2: Engine Core + Frozen Graph + Derived State

实现 WorkflowEngine 的 start_instance（含 frozen_graph 快照 + entity_snapshot）和 advance_instance。实现节点类型分发（approval 创建 task，auto_task 执行 action，end 完成）。实现 ActionRegistry 基础框架。先用线性流程（approval → auto_task → end）验证基本通路。

### Step 3: Parallel Gateway + Timeout Worker

实现并行网关的 join 判断（入边 + 派生查询）。实现后台 Worker（超时扫描、提醒，`FOR UPDATE SKIP LOCKED`）。实现 `workflow_history` 审计记录。实现 auto_task 失败 suspended + RetryAutoTask Admin API。

### Step 4: Async Hook + Action Implementations + Complete gRPC API

实现 WorkflowHook trait registry 和各实体的 hook（事务外异步执行）。实现具体业务 action（从简单的 action 开始，如 create_production_order）。暴露完整的 gRPC 接口。

## Testing

- Repository 层：sqlx test fixture
- Graph Linter：纯 Rust 单元测试（cycle detection、missing nodes、invalid conditions）
- Condition AST：纯 Rust 单元测试，无需数据库
- Engine 层：mock repository 测试引擎逻辑（frozen_graph 从 JSON 反序列化）
- Worker：集成测试超时扫描、`SKIP LOCKED` 防重复
- 端到端：完整的"创建模板 → 发布 → 发起实例 → 审批/超时 → 完成"流程
- 边界测试：驳回后退回、并行节点部分完成、条件分支选择、assignee 解析为零候选人的 fail-closed、fallback 不存在的 suspended 行为
- **auto_task 测试：**
  - 每个 auto_task 独立事务执行，成功即提交
  - 链式 auto_task 逐个独立执行，中间失败仅回滚当前节点
  - auto_task 失败 → 实例 suspended → RetryAutoTask 从断点恢复（前面已完成的保留）
  - RetryAutoTask 前置条件校验（status = suspended + FOR UPDATE 锁）
  - RetryAutoTask refresh_snapshot 刷新快照后重试
  - Action 幂等性验证（重试不重复创建资源）
  - auto_task statement_timeout 生效
  - auto_task 输出变量供后续条件评估使用
  - auto_task 与 approval 混合流程（审批通过 → 自动生成单据 → 继续审批）
  - Action 未注册时 Graph Linter 拒绝发布
- **重点覆盖组合场景（并行 + multi_approval=all）：**
  - 甲审批通过，乙未操作超时 → auto_reject → 甲的 task cancelled → 实例 reject/terminate
  - 甲审批通过，乙 reject → 立即触发 reject_action → 甲的 task cancelled
  - 甲审批通过，乙审批通过 → 正常推进到 join 节点
  - back_to_previous 退回到 start 或 join 节点时被 Graph Linter 拦截
  - join_progress 与 tasks 表派生查询结果一致性验证（模拟 join_progress 缺失时 fallback 正确性）

## V1 Scope Exclusions

- 拖拽式流程设计器 UI
- 子流程嵌套
- 复杂会签规则（V1 只支持 any 和 all）
- auto_task 重试策略（仅支持手动重试，不支持自动重试 + 退避）
