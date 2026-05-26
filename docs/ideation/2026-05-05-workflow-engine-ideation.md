---
date: 2026-05-05
topic: workflow-engine-design
focus: docs/superpowers/specs/2026-05-05-workflow-engine-design.md
mode: repo-grounded
---

# Ideation: Workflow Engine Design Improvements

## Grounding Context

**Codebase Context:** ABT 是 Rust + PostgreSQL + gRPC 的 BOM 与库存管理系统。5 crate workspace，严格 4 层架构（proto→model→repo→service→implt→handler）。18 个 repository，17 个 service 实现。使用 sqlx raw SQL，JSONB metadata 列，soft delete，全局 AppContext singleton。

**Past Learnings:** (1) 状态转换必须用 SELECT FOR UPDATE，(2) fail-closed 权限检查，(3) 三层错误处理，(4) 应用层引用检查优于 FK，(5) 幂等迁移 + 回滚，(6) 尽早扩展 proto 权限枚举。

**Design Spec:** 自建 DB 驱动的工作流引擎，5 张表（templates, nodes, edges, instances, tasks），JSONB 配置审批人规则，条件边路由，并行网关支持。V1 排除拖拽设计器、定时器、子流程。

## Ranked Ideas

### 1. 将节点和边折叠到模板 JSONB 中（5表→3表）

**Description:** 不使用独立的 `workflow_nodes` 和 `workflow_edges` 表，而是将整个图定义存储为 `workflow_templates` 上的单个 JSONB 列：`{nodes: [...], edges: [...]}`。V1 没有可视化设计器，图很小（3-8 个节点），并且模板是原子发布的。运行时保持 `workflow_instances` + `workflow_tasks` 不变。

**Warrant:** `reasoned:` ABT 代码库已经对复杂嵌套结构使用 JSONB（`products.meta` 存储 `ProductMeta`）。工作流模板是原子编写的（您不会孤立地编辑一个节点——您一次性发布整个模板），并且图很小。JSONB 非常适合原子写入、小型、嵌套的结构。每个 ABT 表都需要模型、仓库、服务、处理器、原型定义和迁移——3 个更少的表意味着大约减少 15 个文件和数百行的样板代码。

**Rationale:** 将 5 表设计减少到 3 表。消除 2 个仓库、2 组 CRUD 接口和跨表 JOIN 以进行模板加载。模板版本控制变得更简单——一个版本就是它的 JSONB blob。

**Downsides:** 不能查询单个节点/边；V2 可视化设计器可能需要将其重新规范化。通过 JSONB `->>` 操作符进行迁移仍然足够。

**Confidence:** 85%
**Complexity:** Low（减少工作）

---

### 2. 在实例创建时冻结图快照

**Description:** 在 `start_instance` 时，将完整的节点+边图序列化为实例行上的 `frozen_graph` JSONB 列。实例不再在运行时 `JOIN` 模板/节点/边表。模板编辑从不影响正在运行的实例。

**Warrant:** `direct:` BOM 系统在创建时快照其自己的节点结构（`bom_nodes` 是 BOM 自己的子节点副本）。产品设计已经需要快照语义，它只是通过版本+状态守卫间接地实现了这一点。`bom_labor_cost` 使用 `lock_and_get_unit_prices` + 写入冻结价格——一种 ABT 中成熟的快照模式。

**Rationale:** 运行时图遍历从 3 个以上的表读取变为 1 行读取。使引擎可测试——无需数据库即可在静态结构上进行单元测试图遍历。消除了"模板在实例运行中途更改"这类错误。

**Downsides:** 每个实例都会占用一些存储空间。丢失模板实例的追溯性（哪个模板版本生成了这个实例？），尽管您可以在快照中存储 `template_id` + `version`。

**Confidence:** 90%
**Complexity:** Low

---

### 3. 用派生的待处理任务查询替换 `current_node_ids` 数组

**Description:** 移除 `workflow_instances` 上的 `current_node_ids UUID[]`。相反，通过 `SELECT node_id FROM workflow_tasks WHERE instance_id = $1 AND status = 'pending'` 来派生"当前节点"。"当前活跃节点"就是待处理任务集——不需要冗余的可变数组。

**Warrant:** `direct:` 劳动流程的并发修复将"先读后写"模式记录为需要 `SELECT FOR UPDATE`。`current_node_ids` 数组修改正是这种模式，但在并行任务共享的一行上。该规范在 `instances.current_node_ids`（可变）和 `tasks.status = 'pending'`（事实来源）中存储了冗余状态。

**Rationale:** 消除了在并行网关合并期间的行级争用。消除了一类同步 bug。在实例上不需要 `SELECT FOR UPDATE`——任务行是独立更新的，并且"当前状态"始终是来自任务的派生查询。

**Downsides:** 状态查询略有增加（通过 `instance_id` 上已索引的 `workflow_tasks` 进行 JOIN/子查询）。对于 V1 规模（每个实例少于 50 个任务），这可以忽略不计。

**Confidence:** 95%
**Complexity:** Low（删除代码）

---

### 4. 用基于 Rust 特性的钩子注册表替换 JSONB 回调

**Description:** 不使用带有 `on_approved: {action: "update_entity_status"}` 的 JSONB，而是为每个 `entity_type` 定义一个 `WorkflowHook` 特性，包含 `on_approved(instance, entity)` 和 `on_rejected(instance, entity)` 方法。模板配置只表示 `has_callback: true`。引擎根据 `entity_type` 分派到注册的钩子。

**Warrant:** `direct:` ABT 代码库在任何地方都使用基于特性的分派——`BomService`、`ProductService` 都是带有具体实现（impl）的异步特性。JSONB 回调意味着引擎必须解析 `action` 字符串并在 Rust 函数上进行匹配——这很脆弱，且无法在编译时测试。ABT 三层错误处理要求回调层成为错误链的一部分，而不是一个事后才考虑的字符串匹配。

**Rationale:** 在编译语言中，基于字符串的回调分派违背了编译时安全的目的。拼写错误会默默失败。基于特性的注册表提供了编译时保证。使测试变得简单——模拟（mock）钩子特性，而不是解析 JSON。

**Downsides:** 每次新实体类型与工作流集成时都需要 Rust 代码更改。但实体类型数量有限（产品、BOM、采购单），因此这种权衡是值得的。

**Confidence:** 80%
**Complexity:** Medium

---

### 5. 带有编译时验证的条件 AST

**Description:** 不使用原始 JSONB，如 `{"field": "amount", "op": ">", "value": 10000}`，而是定义一个 Rust 基于枚举的表达式树（`Condition::And(vec![...])`、`Condition::FieldCompare { ... }`）。通过类型化的 `ConditionEvaluator` 进行评估。添加一个 `Condition::Permission { role, action }` 变体，以重用现有的 RBAC 系统。条件在模板创建时进行验证，而不是在转换失败时。

**Warrant:** `direct:` 产品表重新设计正在修复完全相同类型的 bug——`meta->>'category' = term_id.to_string()` 是一个文本与整数的比较，它默默匹配了零行。JSONB 条件评估将重现相同的故障模式。在 50,000 元的采购订单中，将字符串"50000"与数字 10000 进行静默比较可能会绕过首席财务官的审批。

**Rationale:** 一旦您拥有 15 个带有嵌套条件的模板，JSONB 条件就变得难以维护。AST 强制在写入时进行验证，并使条件可以独立测试。包括 RBAC 条件在内，每个新的条件类型都是一个枚举变体。

**Downsides:** 前期设计工作量高于扁平的 JSONB 映射。条件枚举（enum）需要仔细设计以处理嵌套逻辑。

**Confidence:** 75%
**Complexity:** Medium

---

### 6. 失败即关闭的被指派者解决方案，并提供明确的升级路径

**Description:** 当被指派者规则解析为零个候选人时（例如，部门已重组，"department_manager"不再存在），工作流必须失败即关闭（任务分配失败、流程暂停并发出警报），而不是失败即开放（跳过审批）。定义明确的升级路径：每个节点配置都必须包含一个 `fallback_assignee` 或一个 `escalation_timeout`，以防止永久性阻塞。

**Warrant:** `direct:` 权限缓存事件文档记录："关键的安全初始化必须以 `.expect()` 失败即关闭，永远不能在空数据的情况下默默继续。"工作流被指派者解析是其双重问题。V1 设计有 4 种被指派者类型（角色、用户、部门负责人、发起人经理）——其中 3 种可能解析为空。

**Rationale:** 如果在重组后，"department_manager"解析为零个候选人，那么该部门的每个采购单都将永久阻塞或跳过审批。两者在没有明确的升级设计的情况下都是不可接受的。这是 V1 的安全关键设计决策。

**Downsides:** 每个节点配置都需要考虑升级。增加了模板定义的复杂性。

**Confidence:** 90%
**Complexity:** Low

---

### 7. 事件源工作流日志，带有订阅者模式

**Description:** 将工作流日志设计为一个仅追加的事件流：`(event_id, instance_id, event_type, payload, caused_by, occurred_at)`。每个转换、分配和完成都是一个事件。公开一个通用的 `WorkflowEventSubscriber` 特性，以便其他模块（通知、分析、合规性报告）可以做出反应，而无需耦合到引擎。

**Warrant:** `external:` ABT 代码库已经使用 `operator_id` 审计追踪和软删除——团队重视可追溯性。订阅者模式意味着当有人需要"当采购订单审批停滞 3 天时发送通知"时，他们实现一个 `EventSubscriber`，而不是修改工作流引擎。引擎永远不会知道通知的存在。

**Rationale:** 防止引擎发展成上帝对象。SLA 监控、通知、仪表盘更新和合规性报告都成为订阅事件的独立模块。每个新的消费者都不会给引擎核心增加复杂性。

**Downsides:** 事件 schema 成为一个公共契约。比仅状态追踪需要更多的存储空间。当您需要确定性重放时，事件溯源会增加读取复杂性。

**Confidence:** 70%
**Complexity:** Medium-High

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|----------------|
| 1 | Transaction boilerplate macro | Orthogonal to workflow engine; separate DX improvement |
| 2 | 25 new files for 5 tables | Merged into survivor #1 (collapse to 3 tables) |
| 3 | Workflow audit table design | Merged into survivor #7 (event sourcing) |
| 4 | Executor type read/write distinction | Pre-existing pattern issue, not workflow-specific |
| 5 | Pure SQL trigger engine | Subject-replacement: eliminates Rust engine |
| 6 | Pull-based task assignment | High behavioral complexity for V1 |
| 7 | Conditions as SQL WHERE | Merged into survivor #5 (Condition AST) |
| 8 | No gateway concept | Too vague without concrete alternative |
| 9 | Advisory lock + CAS | Less grounded than SELECT FOR UPDATE pattern |
| 10 | Templates in code, not DB | Contradicts approved "configurable flows" requirement |
| 11 | DB triggers for task creation | Moves engine logic to DB, contradicts design |
| 12 | No workflow engine at all | Subject-replacement |
| 13 | Kill entity_type/entity_id polymorphism | Contradicts approved polymorphic design |
| 14 | WorkflowEngine as singleton actor | Changes fundamental service pattern unnecessarily |
| 15 | Assignee resolution to caller | Over-engineers for V1 |
| 16 | Generic WorkflowEntity trait | Premature abstraction for 3-4 entity types |
| 17 | Multi-entity bindings registry | V1 only needs single-entity association |
| 18 | Immutable template versions | Overlaps with survivor #2 (frozen graph) |
| 19 | Unified expression engine | Merged into survivor #5 |
| 20 | Pluggable TransitionLock trait | Premature for V1 single-service deployment |
| 21 | Railway interlocking | Practical recommendation covered by concurrency ideas |
| 22 | Git merge strategies | Simple config field, not separate architectural idea |
| 23 | SSA append-only state | Conflicts with mutable-status design |
| 24 | Kanban WIP limits | Below meeting-test for V1 |
| 25 | DNS zone delegation | V2 subprocess concern |
| 26 | Declarative pattern matching | Merged into survivor #5 |
| 27 | Chess NodeHandler trait | Premature for V1's 2-3 node types |
| 28 | Circuit breaker | Adds complexity for non-existent problem |
| 29 | Zero-template ad-hoc flow | Subject-replacement |
| 30 | Everything is approval | Impractical thought experiment |
| 31 | Synchronous workflow | Human approvals inherently async |
| 32 | CRDT offline workflow | Unrealistic for V1 |
| 33 | Per-instance template mutation | Frozen graph is simpler |
| 34 | State-table routing (no edges) | Less capable than DAG model |
| 35 | Pure PostgreSQL engine | Subject-replacement |
| 36 | Immutable template stream | Merged into survivor #2 |

---

## Round 2: Implementation Improvements (2026-05-17)

**Mode:** repo-grounded
**Focus:** 工作流引擎实施计划的改进构思
**Topic Axes:** Engine runtime safety, Developer experience, Data model & storage, Operational readiness, Extensibility & integration

48 个候选想法生成（6 框架 × 6-8 想法），去重后约 35 个独立概念，7 个存活。

### Survivors (Adopted)

#### 8. frozen_graph Schema Versioning

**Description:** `frozen_graph` JSONB 信封中加入 `graph_version: u32` 字段。引擎按版本分发到对应的反序列化路径，防止代码演进破坏运行中实例。
**Axis:** Data model & storage
**Basis:** `direct:` 代码库已多次遭遇 JSONB 类型演进问题（products.meta 无演进策略、权限缓存迁移数据丢失）。工作流实例可存活数周数月，引擎 Rust 类型演进会破坏 frozen_graph 反序列化。
**Rationale:** 工作流引擎将长期运行实例和代码演进绑定。版本化使每次演进成本恒定。
**Downsides:** 需维护多版反序列化逻辑，但 V1 只需一版。
**Confidence:** 90% | **Complexity:** Low
**Status:** Explored → Adopted as Improvement 8

#### 9. ActionRegistry Startup Validation

**Description:** 所有 action 注册完成后，启动时校验所有 active 模板引用的 action 已注册。任一未注册 → 拒绝启动（fail-closed）。
**Axis:** Operational readiness
**Basis:** `direct:` 权限缓存 fail-open 事件的教训（"OnceLock 失败必须 fail-closed"）。部署时意外删除 action 注册 = 潜伏炸弹。
**Rationale:** 在部署时发现问题比运行时发现便宜得多。
**Downsides:** 启动时多一次 SQL 查询，V1 模板数量极少无影响。
**Confidence:** 90% | **Complexity:** Low
**Status:** Explored → Adopted as Improvement 9

#### 10. PostgreSQL CHECK Constraints for Status

**Description:** 为 instances/tasks/templates 的 status 列添加 CHECK 约束，限定为闭合词汇。
**Axis:** Engine runtime safety
**Basis:** `direct:` migration 029 已为 bom.status 建立 CHECK 先例。工作流状态拼写错误会导致实例永久卡死。
**Rationale:** 数据库级约束是从任何入口点保护状态完整性的唯一手段。
**Downsides:** 新增状态值需 ALTER TABLE，但状态枚举变化频率极低。
**Confidence:** 95% | **Complexity:** Low
**Status:** Explored → Adopted as Improvement 10

### Rejected (Round 2)

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 37 | Worker Liveness Heartbeat | Merged into TaskScheduler reuse — ScheduledTask already provides health status |
| 38 | EntitySnapshot Version Conflict | RecordEntityChange + business review adequate for V1 |
| 39 | Instance Deadlock Prevention | Plan already specifies lock ordering; advisory locks add complexity |
| 40 | Lazy Timeout Enforcement | Too radical — adds latency to user-facing paths |
| 41 | Universal Node = Typed Action Bag | V1 has 4 node types; YAGNI |
| 42 | PL/pgSQL Graph Validation | Fragile and hard to test; Rust Graph Linter is better |
| 43 | Event Sourcing From Tasks | Too radical for V1; needs simple CRUD |
| 44 | Deterministic Assignee = Pure Function | Impractical — role queries need DB |
| 45 | YAML Scenario Harness | Orthogonal to engine design; build incrementally |
| 46 | Single-Node Transaction | Plan already specifies for auto_task; approval needs shared tx |
| 47 | JSONB $schema Auto-Migration | Merged into #8 (version dispatch simpler) |
| 48 | Node-level Advisory Locks | V1 scale doesn't warrant; instance FOR UPDATE is simpler |
| 49 | Separate Decision Audit from Op Log | V1 volume small; partition later |
| 50 | Reject One-Template-Per-Entity-Type | V1 explicit scope; add TemplateResolver later |
| 51 | Remove join_progress from Context | Plan has fallback; derived queries add DB load |
| 52 | Template Append-Only Versioning | Current lifecycle works for V1 |
| 53 | Structured Event Bus | High leverage but orthogonal; separate initiative |
| 54 | Deterministic Replay Harness | Premature for V1 |
| 55 | Idempotency Key Enforcement | FOR UPDATE + SKIP LOCKED adequate for V1 |
| 56 | Generic Worker Framework | Merged into TaskScheduler reuse |
| 57 | Entity Change Capture Table | Separate initiative; RecordEntityChange simpler for V1 |
| 58 | WAL Command Journal | Over-engineering; independent transactions provide checkpoint recovery |
| 59 | ECS Component Architecture | Premature for 4 node types; YAGNI |
| 60 | Circuit Breaker for Actions | Actions are DB-only per design constraint |
| 61 | Multi-Pass Validation | Graph Linter already provides this |
| 62 | Kanban WIP Limits | Premature for V1 scale |
| 63 | MVCC Optimistic Concurrency | FOR UPDATE simpler and correct for V1 |
| 64 | Andon Cord Mechanism | Suspended + admin APIs already provide escalation |
| 65 | Dead Path Elimination | V1's 3-8 node graphs don't warrant |
| 66 | Instance-Local Replay Log | Over-engineering for V1 |
| 67 | Template Version Sandboxing | CreateTemplateVersion already provides this |
| 68 | Zero-Worker (pg_cron/NOTIFY) | Adds PostgreSQL extension dependency |
| 69 | Auto_Task Dry-Run Gate | Can be added per-action later |
| 70 | Context Diff Checkpointing | V1 context small enough for full replacement |
| 71 | Instance Fingerprinting | V1 unlikely to have concurrent starts; add when needed |
| 72 | Embedded Lint-as-You-Go | Structural checks belong in Graph Linter, not Deserialize |
| - | axis: Extensibility & integration | No direct survivor — all extensibility ideas were too radical or V2 concerns |
