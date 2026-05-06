---
title: "feat: Workflow Engine"
type: feat
status: active
date: 2026-05-06
origin: docs/superpowers/specs/2026-05-05-workflow-engine-design.md
---

# feat: Workflow Engine

## Summary

为 ABT 系统实现嵌入式工作流引擎，支持可配置审批流程（approval 节点）、系统自动执行动作（auto_task 节点）、并行网关（join 节点）、条件路由（Condition AST）和超时调度。引擎完全集成在现有 Rust + PostgreSQL + gRPC 架构中，4 张表，零外部依赖。

---

## Problem Frame

ABT 系统目前没有工作流支持。业务流程（如采购审批 → 生成生产定单 → 生产排程 → 领料 → 成本入库 → 出货）全靠人工协调。需要可配置的工作流引擎来自动驱动这些流程。

---

## Requirements

- R1. 模板管理：CRUD + 发布（draft → active → archived），graph 存为 JSONB
- R2. Graph Linter：发布时强制校验（start/end 节点、环检测、fallback_assignee、action 注册校验）
- R3. Condition AST：Rust enum 表达式树，模板创建时验证，运行时求值
- R4. 实例管理：start_instance 按实体类型查找活动模板，冻结图快照，entity_snapshot
- R5. 审批流转：advance_instance，多签（any/all），reject 处理（terminate/back_to_previous）
- R6. auto_task：每个节点独立事务执行，ActionRegistry trait 注册，失败 suspended + 手动重试
- R7. 并行网关：join 节点入边汇聚，join_progress 跟踪
- R8. 超时 Worker：后台扫描 + FOR UPDATE SKIP LOCKED，timeout_action（auto_approve/auto_reject/escalate/notify）
- R9. WorkflowHook：事务后异步执行，失败记录 + RetryFailedHook Admin API
- R10. RecordEntityChange：流程运行期间实体变更通知，不暂停/不取消
- R11. gRPC API：完整的 proto 定义和 handler 实现

**Origin flows:** F1 (模板定义→发布), F2 (发起实例→审批→auto_task→完成), F3 (超时处理), F4 (驳回/退回), F5 (实体变更通知)

---

## Scope Boundaries

- 拖拽式流程设计器 UI
- 子流程嵌套
- 复杂会签规则（V1 只支持 any 和 all）
- auto_task 自动重试（仅手动 RetryAutoTask）
- 可视化流程图渲染

### Deferred to Follow-Up Work

- 具体业务 Action 实现（create_production_order 等）：各业务模块独立实现
- 前端工作流管理界面：独立前端项目
- 通知系统集成（邮件/消息推送）：需对接外部通知服务

---

## Context & Research

### Relevant Code and Patterns

- **分层模式**：proto → model → repo → service trait → service impl → gRPC handler
- **Proto 编译**：`abt-grpc/build.rs` 自动扫描 `proto/abt/v1/*.proto`，放入新 proto 文件即可
- **AppState**：`abt-grpc/src/server.rs` 中 `OnceCell<Arc<AppState>>` 单例，每个服务通过工厂方法获取
- **工厂方法**：`abt/src/lib.rs` 中 `get_*_service(ctx)` → `impl crate::service::*Service`
- **事务模式**：`pool.begin()` → repo 方法接受 `Executor<'_>` → `tx.commit()`
- **JSONB 模式**：`serde_json::json!()` 序列化 + `$N::jsonb` 绑定
- **ID 类型**：全代码库使用 `i64` / `BIGSERIAL`，proto 用 `int64`
- **并发保护**：已建立 `SELECT ... FOR UPDATE` 模式（见 labor-process 文档）

### Institutional Learnings

- **读后写必须加锁**：任何读取状态再写入的模式必须用 `SELECT FOR UPDATE`（`docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`）
- **OnceLock 失败必须 fail-closed**：单例初始化失败不应回退到空状态（`docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`）
- **业务错误用 business_error()**：gRPC 三层错误处理，状态转换失败用 `business_error()` 不污染日志（`docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`）
- **不用数据库外键**：应用层做引用检查，不用 FK 约束

---

## Key Technical Decisions

- **使用 i64 而非 UUID**：匹配全代码库的 BIGSERIAL/int64 约定，避免引入 uuid crate
- **Graph 存为 JSONB**：与 `products.meta` 模式一致，用 `serde_json::Value` 表示
- **Condition AST 用 serde enum**：`#[serde(tag = "type")]` 标签联合体，与 JSONB 互转
- **不引入 petgraph**：DFS 遍历和环检测自行实现（图很小，3-8 节点）
- **不引入 sha2**：用简单哈希或跳过 graph_checksum V1 实现（用 JSON 规范化的字符串比较即可）
- **Worker 用 tokio::spawn**：无现有后台任务模式，用 `tokio_util::sync::CancellationToken` 控制生命周期（需新增依赖）
- **ActionRegistry 在 AppContext 初始化**：各业务模块在 `init_context_with_pool()` 后注册 action
- **每个 auto_task 独立事务**：成功即提交，失败仅回滚当前节点，重试从断点继续

---

## Open Questions

### Resolved During Planning

- **ID 类型**：使用 i64 匹配代码库约定
- **auto_task 事务模型**：独立事务（非共享链式事务），失败 suspended + RetryAutoTask
- **实体变更处理**：RecordEntityChange 不暂停工作流，仅通知当前处理人
- **外部依赖**：不引入 petgraph/sha2，自行实现简单图算法

### Deferred to Implementation

- **auto_task 的 statement_timeout 具体值**：默认 5 秒，在 engine 初始化时配置
- **Worker 扫描频率的动态调整 API**：先用环境变量，Admin API 后续再加
- **EntitySnapshotProvider 的具体实现**：各业务模块注册，engine 不关心细节

---

## Output Structure

```
proto/abt/v1/workflow.proto                    ← gRPC 服务定义
abt/src/models/workflow.rs                     ← 数据模型 + Graph 类型 + Condition AST
abt/src/repositories/workflow_template_repo.rs ← 模板 CRUD
abt/src/repositories/workflow_instance_repo.rs ← 实例 CRUD
abt/src/repositories/workflow_task_repo.rs     ← 任务 CRUD
abt/src/repositories/workflow_history_repo.rs  ← 审计历史
abt/src/service/workflow_service.rs            ← Service trait
abt/src/implt/workflow_engine.rs               ← 核心引擎（start_instance, advance_instance）
abt/src/implt/workflow_actions.rs              ← AutoAction trait + ActionRegistry
abt/src/implt/workflow_hooks.rs                ← WorkflowHook trait + HookRegistry
abt/src/implt/workflow_worker.rs               ← 超时扫描 Worker
abt/src/implt/graph_linter.rs                  ← Graph Linter（发布校验）
abt-grpc/src/handlers/workflow.rs              ← gRPC handler
abt/migrations/039_create_workflow_tables.sql   ← 4 张表 + 索引
```

---

## Implementation Units

### Phase 1: Foundation

- U1. **Database Migration**

**Goal:** 创建 4 张工作流表（templates, instances, tasks, history）及所有索引

**Requirements:** R1, R4（表结构支撑）

**Dependencies:** None

**Files:**
- Create: `abt/migrations/039_create_workflow_tables.sql`

**Approach:**
- 使用 BIGSERIAL 主键（i64）匹配代码库约定
- 不使用数据库外键（遵循应用层引用检查约定）
- graph、frozen_graph、context、suspended_reason、result 均为 JSONB
- status 列用 VARCHAR 存枚举值
- 包含 spec 中定义的全部 5 个索引
- 遵循迁移安全约定：使用 `CREATE TABLE IF NOT EXISTS`、`CREATE INDEX IF NOT EXISTS`

**Patterns to follow:** `abt/migrations/003_create_warehouse_table.sql` — CREATE TABLE IF NOT EXISTS + CREATE INDEX IF NOT EXISTS

**Test scenarios:**
- Happy path: migration 应用成功，4 张表存在，索引存在
- 幂等性: 重复运行不报错

**Verification:** `cargo build` 通过，sqlx 编译时查询验证通过

---

- U2. **Proto Definitions + Models + Graph Types**

**Goal:** 定义 workflow.proto gRPC 服务，创建 Rust 数据模型（含 Graph JSONB 类型、Condition AST）

**Requirements:** R1, R2, R3, R11

**Dependencies:** U1

**Files:**
- Create: `proto/abt/v1/workflow.proto`
- Create: `abt/src/models/workflow.rs`
- Modify: `abt/src/models/mod.rs`

**Approach:**
- Proto 定义所有 spec 中列出的 RPC（Template CRUD, Instance 管理, Task 操作, Admin API）
- Proto message 的 ID 字段用 `int64` 匹配 i64 约定
- graph JSONB 映射为 Rust struct（`WorkflowGraph { nodes: Vec<WorkflowNode>, edges: Vec<WorkflowEdge> }`），用 serde 序列化
- Condition AST 用 serde enum：`Condition::And(vec), .Or(vec), .Not(box), .FieldCompare{..}, .Always, .Never`
- Node config 用 `serde_json::Value`（灵活 JSONB）
- 模型层包含 `WorkflowTemplate`, `WorkflowInstance`, `WorkflowTask`, `WorkflowHistory` 结构体
- 枚举类型用 Rust enum + `FromStr`/`Display`（不引入 strum）
- 在 `proto/abt/v1/base.proto` 中检查是否需要新的通用 message 类型

**Execution note:** 先写 proto，`cargo build` 生成 Rust 代码后再写 model

**Patterns to follow:**
- Proto: `proto/abt/v1/term.proto` — service/message 命名风格
- Model: `abt/src/models/term.rs` — derive Serialize/Deserialize, FromRow
- Model export: `abt/src/models/mod.rs` — `mod workflow; pub use workflow::*;`

**Test scenarios:**
- Happy path: Condition AST 反序列化各种条件类型（And/Or/FieldCompare/Always/Never）
- Happy path: WorkflowGraph 从 JSON 反序列化包含 start/approval/join/auto_task/end 节点
- Edge case: 空 graph 反序列化
- Edge case: Condition AST 嵌套深度 3 层

**Verification:** `cargo build` 通过，proto 编译成功，model 可序列化/反序列化

---

- U3. **Repositories**

**Goal:** 实现 4 个 repo（template, instance, task, history）的全部数据库查询

**Requirements:** R1, R4, R5, R8, R10

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/repositories/workflow_template_repo.rs`
- Create: `abt/src/repositories/workflow_instance_repo.rs`
- Create: `abt/src/repositories/workflow_task_repo.rs`
- Create: `abt/src/repositories/workflow_history_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

**Approach:**
- 每个 repo 用 `pub struct XxxRepo;` 无字段模式
- 方法接受 `Executor<'_>`（支持事务传入）或 `&PgPool`（只读查询）
- Template repo: insert, update(graph/name, 仅 draft), find_by_id, find_active_by_entity_type, publish（status→active）, archive, clone_as_draft（版本克隆）
- Instance repo: insert, find_by_id, find_for_update（SELECT FOR UPDATE）, update_status, update_context, update_join_progress, find_by_entity
- Task repo: insert, find_for_update, update_status_and_action, count_pending_by_node, find_pending_by_instance, find_overdue_pending（FOR UPDATE SKIP LOCKED）
- History repo: insert, list_by_instance
- SQL 用 sqlx `query_as` + `FromRow` derive
- JSONB 列用 `serde_json::Value` 类型，Rust 侧手动反序列化到强类型
- 导出所有 repo struct 和关键行类型到 `mod.rs`

**Patterns to follow:**
- `abt/src/repositories/term_repo.rs` — struct 无字段，静态方法
- `abt/src/repositories/inventory_cascade_repo.rs` — JSONB 处理模式
- `abt/src/repositories/mod.rs` — `mod xxx_repo; pub use xxx_repo::XxxRepo;`

**Test scenarios:**
- Happy path: template CRUD 全流程（insert → find → update → publish → archive）
- Happy path: start_instance 写入 instance + 初始 tasks + history 记录
- Edge case: find_active_by_entity_type 返回 None（无 active 模板）
- Edge case: find_for_update 锁定行
- Edge case: count_pending_by_node 返回正确计数

**Verification:** `cargo clippy` 通过

---

- U4. **Graph Linter + Condition AST Evaluation**

**Goal:** 实现模板发布时的图结构校验，和 Condition AST 运行时求值

**Requirements:** R2, R3

**Dependencies:** U2

**Files:**
- Create: `abt/src/implt/graph_linter.rs`
- Modify: `abt/src/implt/mod.rs`

**Approach:**
- Graph Linter 作为纯函数模块：`pub fn lint_graph(graph: &WorkflowGraph, action_registry: &ActionRegistry) -> Result<()>`
- 校验规则（共 9 条，按 spec）：
  1. 有且仅有一个 start，至少一个 end
  2. DFS 环检测（用 HashSet visited 跟踪，允许合法汇聚）
  3. 每个 approval 节点必须有 fallback_assignee
  4. 每个 auto_task 节点必须有 action，且 action 在 ActionRegistry 中已注册
  5-6. Condition AST 字段白名单校验（V1 可简化为只校验字段非空）
  7. join 节点入边数量匹配
  8. back_to_previous 只能配置在入边源全部为 approval 且唯一入边的节点
  9. graph_checksum 计算（V1 用 JSON 字符串规范化哈希，或跳过）
- Condition AST 求值：`pub fn evaluate(condition: &Condition, ctx: &EvaluationContext) -> bool`
- EvaluationContext 从 instance.context 构建
- CompareOp 对 serde_json::Value 做类型安全的比较（数字比数字、字符串比字符串）

**Execution note:** 纯 Rust 单元测试，不需要数据库

**Patterns to follow:** `abt/src/implt/` 目录下的 impl 模块模式

**Test scenarios:**
- Happy path: 有效线性图（start → approval → end）通过校验
- Happy path: 有效并行图（start → approval×2 → join → end）通过校验
- Error path: 缺少 start 节点 → 返回错误
- Error path: 缺少 end 节点 → 返回错误
- Error path: 图中有环 → 返回错误
- Error path: approval 节点缺少 fallback_assignee → 返回错误
- Error path: auto_task 的 action 未注册 → 返回错误
- Error path: back_to_previous 配置在多入边节点 → 返回错误
- Happy path: Condition Always → true, Never → false
- Happy path: Condition FieldCompare amount > 10000 与 entity_snapshot 求值
- Edge case: Condition And/Or 嵌套组合
- Edge case: EvaluationContext variables 为空时 FieldCompare 引用不存在的字段

**Verification:** 所有 linter 和 condition 单元测试通过

---

### Phase 2: Engine Core

- U5. **Service Trait + Engine Core (Linear Flow)**

**Goal:** 定义 WorkflowService trait，实现 start_instance 和 advance_instance 的基础线性流程（approval → end）

**Requirements:** R4, R5

**Dependencies:** U2, U3, U4

**Files:**
- Create: `abt/src/service/workflow_service.rs`
- Modify: `abt/src/service/mod.rs`
- Create: `abt/src/implt/workflow_engine.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`（添加工厂方法）

**Approach:**
- WorkflowService trait 定义核心方法：start_instance, advance_instance, cancel_instance, get_instance, get_my_tasks 等
- WorkflowEngine struct 持有 `Arc<PgPool>`、`ActionRegistry`、`HookRegistry`
- start_instance 实现：
  1. 按实体类型查找活动模板
  2. 冻结图快照（深拷贝 JSONB）
  3. 获取实体快照（通过 EntitySnapshotProvider registry）
  4. 创建实例记录
  5. 找 start 出边，对目标节点按类型处理（approval → 创建 task，auto_task → 委托 execute_auto_task）
  6. 记录 history
- advance_instance 基础版：
  1. 锁定 task（FOR UPDATE）→ 锁定 instance（FOR UPDATE）
  2. 更新 task 状态
  3. 找出边目标节点
  4. 对 approval 节点：解析 assignee（fail-closed）→ 创建 task
  5. 对 end 节点：标记 completed → 触发 on_approved hook
  6. 记录 history
- assignee 解析：按 assignee_type 查找候选人（role → 查角色用户表，user → 直接用，department_head → 查部门负责人，initiator_manager → 查发起人上级）
- fail-closed：候选人列表为空 → 用 fallback_assignee → fallback 也不存在 → suspended
- 工厂方法注册到 `lib.rs`

**Patterns to follow:**
- Service trait: `abt/src/service/term_service.rs` — `#[async_trait]` + `Send + Sync`
- Service impl: `abt/src/implt/term_service_impl.rs` — `pub struct XxxServiceImpl { pool: Arc<PgPool> }`
- Factory: `abt/src/lib.rs` — `pub fn get_workflow_service(ctx: &AppContext) -> impl crate::service::WorkflowService`

**Test scenarios:**
- Happy path: start_instance 创建实例 + pending task（线性流程）
- Happy path: advance_instance approval → 推进到 end → completed
- Error path: start_instance 找不到 active 模板 → 错误
- Error path: advance_instance 非 pending task → 错误
- Edge case: assignee 解析为空 → fallback_assignee
- Edge case: fallback_assignee 也不存在 → suspended

**Verification:** `cargo clippy` 通过，线性流程（start → approval → end）端到端可运行

---

- U6. **auto_task Execution + ActionRegistry**

**Goal:** 实现 AutoAction trait、ActionRegistry、引擎的 auto_task 执行和 RetryAutoTask

**Requirements:** R6

**Dependencies:** U5

**Files:**
- Create: `abt/src/implt/workflow_actions.rs`
- Modify: `abt/src/implt/workflow_engine.rs`
- Modify: `abt/src/implt/mod.rs`

**Approach:**
- AutoAction trait：`async fn execute(&self, tx: &mut Transaction, instance: &WorkflowInstance) -> Result<ActionOutput>`
- ActionOutput：`variables_update: HashMap<String, serde_json::Value>`
- ActionRegistry：`HashMap<String, Arc<dyn AutoAction>>`，提供 register/get/is_registered
- execute_auto_task 方法：
  1. 开新事务
  2. SET LOCAL statement_timeout（从 config.timeout_seconds 取，默认 5s）
  3. 调用 action.execute()
  4. 成功 → commit + 更新 context.variables + 记录 history → 评估出边继续推进
  5. 失败 → rollback → 独立事务 suspend instance → 返回错误
- advance_from_node：处理链式 auto_task（下一节点也是 auto_task 时继续执行）
- RetryAutoTask：校验 status=suspended + suspended_reason.node_id 匹配 + FOR UPDATE 锁 → 可选 refresh_snapshot → 重新执行 auto_task
- Engine 持有 ActionRegistry 的 Arc，Graph Linter 通过 is_registered 校验

**Test scenarios:**
- Happy path: 单个 auto_task 执行成功 → 推进到下一节点
- Happy path: 链式 auto_task（task1 → task2 → end）逐个独立事务执行
- Error path: auto_task 执行失败 → instance suspended，已完成的 auto_task 结果保留
- Happy path: RetryAutoTask 恢复 suspended instance → 从失败节点继续
- Error path: RetryAutoTask 对 running instance → 前置条件检查失败
- Edge case: auto_task 输出 variables_update → 后续条件可引用

**Verification:** auto_task 执行、失败 suspended、重试恢复 全流程可运行

---

### Phase 3: Advanced Features

- U7. **Parallel Gateway + Multi-approval + Reject Handling**

**Goal:** 实现并行网关 join 判断、多签审批（any/all）、reject 处理（terminate/back_to_previous）

**Requirements:** R5, R7

**Dependencies:** U5

**Files:**
- Modify: `abt/src/implt/workflow_engine.rs`
- Modify: `abt/src/repositories/workflow_task_repo.rs`（添加批量取消 pending task 方法）

**Approach:**
- advance_instance 扩展：
  - join_progress 更新：task 完成时将 node_id 追加到相关 join 节点的 progress
  - Join 检查：目标节点入边 > 1 时，检查 join_progress 是否包含所有入边源；不满足则跳过（等待）
  - join_progress fallback：若 join_progress 不可用，从 tasks 表派生查询
- multi_approval=all：
  - 创建 task 时按候选人数量创建多个 task（同一 node_id）
  - advance 时检查同 node_id 下 pending 数量，> 0 则不推进
  - reject 时立即触发 reject_action，cancel 同 node_id 其余 pending task
- reject terminate：instance status → rejected
- reject back_to_previous：
  - 找入边的唯一前驱节点
  - 在前驱节点创建新 task
  - Graph Linter 已保证入边唯一性

**Test scenarios:**
- Happy path: 并行 start → approval×2 → join → end 全流程
- Happy path: multi_approval=all，全部通过才推进
- Happy path: multi_approval=all + 一人 reject → 立即触发 reject_action，其余 task cancelled
- Happy path: back_to_previous → 退回前驱节点，创建新 task
- Edge case: 并行分支部分完成 → join 不激活
- Edge case: multi_approval=any，任一通过即推进

**Verification:** 并行审批 + 多签 + 驳回退回 全流程可运行

---

- U8. **Timeout Worker**

**Goal:** 实现后台超时扫描 Worker，支持 timeout_action（auto_approve/auto_reject/escalate/notify）

**Requirements:** R8

**Dependencies:** U5, U3

**Files:**
- Create: `abt/src/implt/workflow_worker.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt-grpc/src/server.rs`（启动 Worker）

**Approach:**
- WorkflowWorker struct：持有 `Arc<PgPool>` + `Arc<WorkflowEngine>` + `CancellationToken`
- run() 循环：
  1. 扫描 overdue pending tasks（`status = 'pending' AND due_at < now()`）→ FOR UPDATE SKIP LOCKED
  2. 对每个 task：按 timeout_action 执行
     - auto_approve：调用 advance_instance(approve)
     - auto_reject：调用 advance_instance(reject) → 可能触发 terminate/back_to_previous
     - escalate：使用 fallback_assignee 创建新 task
     - notify：仅记录 history，task 状态不变（等待人工处理）
  3. 扫描 remind_at 过期的 task → 记录 history（通知由上层消费）
  4. sleep scan_interval
- actor_id 使用 SYSTEM_USER 常量（固定值如 0）
- CancellationToken 优雅关闭：收到信号后等待当前事务完成
- scan_interval 从环境变量 `WORKER_SCAN_INTERVAL_SECS` 读取（默认 60）
- Worker 在 `server.rs` 的 `start_server()` 中 `tokio::spawn` 启动
- 需要新增 `tokio-util` 依赖到 `abt/Cargo.toml`（CancellationToken）

**Patterns to follow:** `tokio::spawn` in `abt-grpc/src/handlers/mod.rs:88`（唯一的 spawn 示例）

**Test scenarios:**
- Happy path: task due_at 过期 → Worker 扫描到 → auto_approve → 推进到下一节点
- Happy path: timeout_action = auto_reject → task rejected → 触发 reject_action
- Happy path: remind_at 过期 → 记录 history 通知事件
- Edge case: FOR UPDATE SKIP LOCKED → 并发 Worker 不重复处理
- Edge case: CancellationToken 触发 → Worker 等待当前事务后退出

**Verification:** Worker 启动后可扫描并处理超时 task

---

### Phase 4: Integration

- U9. **WorkflowHook + RecordEntityChange**

**Goal:** 实现 WorkflowHook trait registry、异步 hook 执行、RecordEntityChange

**Requirements:** R9, R10

**Dependencies:** U5

**Files:**
- Create: `abt/src/implt/workflow_hooks.rs`
- Modify: `abt/src/implt/workflow_engine.rs`
- Modify: `abt/src/implt/mod.rs`

**Approach:**
- WorkflowHook trait：`on_approved(instance, entity_id)` + `on_rejected(instance, entity_id)`
- HookRegistry：`HashMap<String, Arc<dyn WorkflowHook>>`
- Hook 执行时机：advance_instance 事务 commit 后异步执行（tokio::spawn）
- Hook 失败处理：写入 workflow_history（hook_executed, success: false）+ 错误详情
- RetryFailedHook：查找最近的失败 hook 记录 → 重新执行
- RecordEntityChange：
  1. 校验 instance status = running
  2. 写入 workflow_history（entity_changed 事件）
  3. 返回当前 pending task 的 assignee 列表（供调用方发送通知）

**Test scenarios:**
- Happy path: instance completed → on_approved hook 触发
- Happy path: instance rejected → on_rejected hook 触发
- Error path: hook 执行失败 → history 记录失败 → RetryFailedHook 可恢复
- Happy path: RecordEntityChange → history 记录 entity_changed + 返回 assignee 列表
- Edge case: hook 执行期间实例状态已变 → 不影响已 commit 的状态

**Verification:** Hook 触发、失败记录、重试恢复 全流程可运行

---

- U10. **gRPC Handler + Server Registration**

**Goal:** 实现完整 gRPC handler，注册到 server，暴露所有 API

**Requirements:** R11

**Dependencies:** U5, U6, U7, U8, U9

**Files:**
- Create: `abt-grpc/src/handlers/workflow.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/server.rs`

**Approach:**
- WorkflowHandler struct（无字段或持有工厂方法引用）
- 实现 proto 生成的 trait 中的每个 RPC 方法
- 每个方法：解析请求 → 调用 AppState 中获取的 workflow_service → 转换响应
- 权限控制：Template 管理（Create/Update/Publish）需要 admin 权限，Task 操作（Approve/Reject）校验 assignee_id = 当前用户
- 错误映射：
  - anyhow 错误 → gRPC INTERNAL（via err_to_status）
  - 状态校验失败 → gRPC FAILED_PRECONDITION（via business_error）
  - 找不到资源 → gRPC NOT_FOUND
  - suspended → gRPC ABORTED + suspended_reason
- 在 `server.rs` 注册 `AbtWorkflowServiceServer::with_interceptor`
- Model ↔ Proto 转换：为 model 添加 `From<ProtoXxx>` 和 `Into<ProtoXxx>` 实现

**Patterns to follow:**
- Handler: `abt-grpc/src/handlers/term.rs` — `#[require_permission]` + `AppState::get().await` + service 调用
- Server registration: `abt-grpc/src/server.rs` — `add_service(XxxServer::with_interceptor(handler, interceptor))`

**Test scenarios:**
- Happy path: CreateTemplate → 返回 template
- Happy path: PublishTemplate → Graph Linter 校验通过 → status = active
- Happy path: StartInstance → 创建实例 + task
- Happy path: ApproveTask → 推进流程
- Error path: PublishTemplate 图不合法 → INVALID_ARGUMENT
- Error path: ApproveTask 非审批人 → PERMISSION_DENIED
- Error path: StartInstance 无 active 模板 → NOT_FOUND

**Verification:** `cargo clippy` 通过，gRPC 服务可启动并响应请求

---

## System-Wide Impact

- **Interaction graph:** 工作流引擎是一个全新的子系统。它通过 EntitySnapshotProvider 和 WorkflowHook 两个 trait 与现有业务模块（product, bom, inventory）交互。业务模块在初始化时注册 provider 和 hook，引擎不直接依赖任何业务模块。
- **Error propagation:** 引擎内部错误用 anyhow，通过 service trait 向上传播。Handler 层按三层错误约定（err_to_status / validation / business_error）映射到 gRPC status code。
- **State lifecycle risks:** auto_task 独立事务已隔离失败影响。并行审批通过 instance-level FOR UPDATE 锁避免竞态。
- **API surface parity:** 新增独立的 workflow.proto 服务，不影响现有 API。
- **Integration coverage:** 端到端测试需验证：模板创建 → 发布 → 实例启动 → 审批 → auto_task → 完成 全链路。Worker 超时处理需要 SKIP LOCKED 集成测试。
- **Unchanged invariants:** 现有所有 CRUD 服务、gRPC handler、数据库表不受影响。工厂方法模式（lib.rs）新增但不修改已有方法。

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| JSONB graph 反序列化性能 | V1 图很小（3-8 节点），serde_json 足够。100+ 节点时考虑优化 |
| auto_task 长事务 | statement_timeout 强制 5s 上限。每节点独立事务 |
| 并行审批竞态 | instance-level FOR UPDATE 锁，join_progress 优先于派生查询 |
| Worker 单点 | V1 单实例部署。SKIP LOCKED 允许多实例但不推荐 |
| 缺少后台任务经验 | 代码库首次引入 Worker，需关注 CancellationToken 优雅关闭和错误恢复 |
| 新增依赖 tokio-util | 仅用 CancellationToken，无其他 transitive 依赖 |

---

## Phased Delivery

### Phase 1 — Foundation（U1-U4）
- 数据库表、proto、model、repo、Graph Linter、Condition AST
- 全部可独立验证（`cargo build` + `cargo clippy` + 单元测试）
- **里程碑：** 模板 CRUD + Graph Linter 发布校验可运行

### Phase 2 — Engine Core（U5-U6）
- Service trait、引擎核心（线性流程）、auto_task 执行
- **里程碑：** start_instance → approval → auto_task → end 线性流程端到端可运行

### Phase 3 — Advanced Features（U7-U8）
- 并行网关、多签、驳回退回、超时 Worker
- **里程碑：** 并行审批 + 超时自动处理可运行

### Phase 4 — Integration（U9-U10）
- Hook registry、RecordEntityChange、gRPC handler
- **里程碑：** 完整 gRPC API 可调用，全链路端到端可运行

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-05-workflow-engine-design.md](docs/superpowers/specs/2026-05-05-workflow-engine-design.md)
- Related code: `abt/src/repositories/`（repo 模式参考）
- Related code: `abt-grpc/src/server.rs`（服务注册模式）
- Institutional learning: `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`（FOR UPDATE 模式）
- Institutional learning: `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`（gRPC 错误处理）
