# UML 类图设计文档

> 2026-05-22 设计，基于 target.md 蓝图

## 文件说明

### HTML 可视化预览（Mermaid + 缩放/拖拽）

| 文件 | 内容 | 实体数 |
|------|------|--------|
| [00-module-dependencies.html](00-module-dependencies.html) | 模块间接口依赖关系总览 | 9 模块 + 54 Service trait |
| [00-shared-infrastructure.html](00-shared-infrastructure.html) | 共享基础设施层 — 文档编号、文档关联、库存预留、成本账本、领域事件(Outbox)、状态机、审计日志、幂等去重、通知服务、Excel导入导出、定时任务 | 8 核心 + 通知 + Excel + 定时任务 + 9 枚举 |
| [01-sales.html](01-sales.html) | 销售模块 — 报价、订单、发货、退货、对账（DomainError + 业务校验规则 + 语义化状态方法） | 5 主表 + 5 明细表 |
| [02-purchase.html](02-purchase.html) | 采购模块 — 采购报价、订单、退货、对账、付款、零星请购（Supplier 已迁至 Master Data） | 6 主表 + 6 明细表 |
| [03-wms.html](03-wms.html) | 仓储模块 — 三级库位、策略引擎、来料、库存事务、领料、倒冲、盘点、调拨、形态转换、锁库、级联库存查询 | 12 主表 + 10 明细表 + 1 查询服务 |
| [04-mes.html](04-mes.html) | 生产模块 — 计划、工单、生产批次(流转卡)、工序、报工、计件工资、报检、完工入库（委外委托 OM） | 7 主表 + 3 明细表 + 3 枚举 |
| [05-outsourcing.html](05-outsourcing.html) | 委外管理 — 委外单、发料明细、追踪节点、转自制 | 3 主表 + 7 节点类型 |
| [06-qms.html](06-qms.html) | 质量管理 v2.3 — 检验规格、检验结果、MRB不良评审、RMA客诉（QualityGateService独立 + QualityGateStatus + Req/Filter + 乐观锁 + 工作流集成 + execute_disposition + InCallerTx硬门 + Guard Conditions + 幂等约束 + JSONB强类型） | 5 Service + 4 主表 + 10 枚举 + 8 Req + 4 Filter + 3 JSONB类型 |
| [07-fms.html](07-fms.html) | 财务管理 v2 — 日记账、日记账明细、核销、费用报销、成本核算（强类型 Filter + BalanceSummary + 幂等键 + TransactionMode + CounterpartyRef + 工作流解耦） | 5 主表 + 1 明细表 + 4 请求/返回结构体 |
| [08-workflow-engine.html](08-workflow-engine.html) | 工作流引擎 V2 — 依赖共享层事件/状态机，Saga 补偿 + 增强节点 | 3 Service + 4 核心实体 |
| [09-master-data.html](09-master-data.html) | 主数据模块 v5 — 产品目录、分类、价格、BOM、客户(Customer)、供应商(Supplier)、工艺路线(Routing)、工序字典(LaborProcessDict)、劳务工序(BomLaborProcess)（CQRS 拆分 + ServiceContext + 乐观锁 + 客商统一主数据 + 三层工艺解耦） | 13 Service + 28 核心实体 + 16 请求结构体 |

## 查看方式

- **HTML**: 直接在浏览器中打开任意 `.html` 文件，支持滚轮缩放、拖拽平移

## 设计原则

- **接口先行**: 每个 Service trait 定义清晰的输入输出，模块间通过接口交互
- **共享层解耦**: DocumentSequence / DocumentLink / InventoryReservation / CostEntry 通过 DocumentType 枚举解耦
- **分层包结构**: Migration → Model → Repository → Service Trait → Service Impl → Handler → Proto
- **业财一体**: 从第一天起记录成本，避免事后对账
- **委外统一**: 委外供应商 = WMS 虚拟库位，复用已有调拨/入库模型
- **领域事件**: DomainEventBus 跨模块解耦，Outbox 模式 + 异步分发，LISTEN/NOTIFY 低延迟驱动
- **状态机**: StateMachineService 统一管理单据生命周期，转换规则存 DB 可配置
- **Saga 补偿**: 基于增强 WorkflowEngine 的长流程编排，支持失败逆序补偿
- **质量关卡**: 检验不合格自动阻断下游流转（数据层面硬门）
- **双层记账**: 每笔日记账同时记借方和贷方，支持成本中心和利润中心归集

---

## 共享基础设施接口规范

> 实现共享基础设施或业务模块集成共享服务时，必须遵守以下接口规范。
> 详细类图和字段定义见 [00-shared-infrastructure.html](00-shared-infrastructure.html)。

### 服务一览

| 共享服务 | 职责 | 调用时机 |
|----------|------|----------|
| `DocumentSequenceService` | 生成单据编号（如 SO-2026-05-00142） | 创建单据时 |
| `DocumentLinkService` | 记录单据间关联（有向图） | 单据流转时 |
| `InventoryReservationService` | 库存预留/释放/过期（ATP 可用量，`source_line_id` 支持行级精确释放） | 订单确认、工单下达、领料时 |
| `CostEntryService` | 成本累积（双层记账） | 产生成本的业务事件时 |
| `DomainEventBus` | 事件发布（Outbox 模式）| 业务操作完成后 |
| `StateMachineService` | 状态转换管理 | 替代 if-else 状态校验 |
| `AuditLogService` | 操作审计（字段级 diff）| 数据变更时，同事务内 |
| `IdempotencyService` | 幂等去重 | EventProcessor 消费事件前、API 防重复提交 |
| `DeadLetterService` | 死信队列运维 | EventProcessor 超限后查询/重试/归档 |
| `NotificationService` | 跨模块消息通知 | 业务事件触发通知、前端轮询未读数 |
| `ExcelImportService` / `ExcelExportService` | Excel 导入导出框架 | 每个导入/导出操作独立实现 |
| `ScheduledTask` / `TaskSchedulerService` | 定时任务框架 | 后台任务调度（如库存预警） |

### 关键接口签名

#### DomainEventBus — 事件发布

```rust
struct EventPublishRequest {
    event_type: DomainEventType,
    aggregate_type: String,
    aggregate_id: i64,
    payload: serde_json::Value,
    idempotency_key: Option<String>,  // None 时自动生成 "{aggregate_type}:{aggregate_id}:{event_type}"
    // operator_id 从 ServiceContext 自动获取，不在此传入
}

trait DomainEventBus {
    fn publish(ctx, req: EventPublishRequest) -> Result<i64>;
    fn mark_processed(ctx, ids: Vec<i64>) -> Result<u64>;
    fn mark_failed(ctx, id, reason) -> Result<()>;
    fn find_events(ctx, query) -> PaginatedResult<DomainEvent>;
}
```

**Handler 语义**：同步 handler（库存预留、质量关卡）在调用者事务内执行；异步 handler（成本记录、文档链接、Workflow 触发）通过 Outbox 异步消费。

#### AuditLogService — 审计日志

```rust
enum AuditAction {
    Create,
    Update,
    Delete,
    Transition,
}

struct RecordAuditLogReq {
    entity_type: &'static str,
    entity_id: i64,
    action: AuditAction,
    changes: Option<JsonValue>,
    context: Option<JsonValue>,
}

trait AuditLogService {
    fn record(ctx, db, req: RecordAuditLogReq) -> Result<i64>;
    fn query_logs(ctx, query) -> PaginatedResult<AuditLog>;
}
```

`action` 必须使用 `AuditAction` 枚举，不允许裸字符串。`changes` 中敏感字段标记 `sensitive: true`，record 内部自动脱敏。

#### StateMachineService — 状态机

```rust
enum SideEffect {
    PublishEvent { event_type: DomainEventType, payload_template: Value },
    Notify { role_ids: Vec<i64>, template: String },
    TriggerWorkflow { definition_id: i64 },
    UpdateField { field: String, value_template: Value },
}

// StateTransitionDef.side_effects: Vec<SideEffect>
// 存储为 JSONB，代码层类型安全
```

#### BatchResult — 批量操作

```rust
enum BatchMode {
    Atomic,           // 全建或全不建
    ContinueOnError,  // 部分失败继续
}

struct BatchFailure {
    index: i32,
    error: DomainError,  // 复用统一错误模型，不使用裸 String
}

struct BatchResult {
    success_count: i32,
    failed_items: Vec<BatchFailure>,
    total: i32,
    mode: BatchMode,
}
```

每个 `batch_*` 方法固定语义：
- `create_links`: **Atomic**（订单确认时关联要么全建要么全不建）
- `reserve`: **ContinueOnError**（部分行库存不足不影响其他行；单行 qty>ATP 时部分预留 min(qty,ATP)，仅 ATP<=0 整行失败）
- `create_entries`（CostEntry）: **Atomic**（双层记账必须完整）

#### ATP 可用量口径与 Reservation / Lock 边界

> **铁律**：对外可用量（ATP）必须用 `InventoryTransactionService.query_available()`，
> 即 `StockLedger.quantity − InventoryLock − InventoryReservation`。
> **禁止**把 `stock_ledger.available_qty` / `reserved_qty` 反范式字段当作对外可用量读取——
> 这两个字段**仅含 InventoryLock**（质量冻结/客户质押），不含 `InventoryReservation`。

- `InventoryReservationService`（shared 域）：业务软/硬预留，订单确认/工单下达/领料时占用，记 `inventory_reservations` 表。
- `InventoryLockService`（wms 域）：物理整批冻结，记 `stock_ledger.reserved_qty`。
- 两者 **disjoint**：ATP 计算同时扣两者，无重复扣减。扣减顺序：`quantity − Lock(ledger.reserved_qty) − Reservation(inventory_reservations)`。
- 负库存：消耗型事务（`SalesShipment`/`MaterialIssue`/`Scrap`）扣减前在 `InventoryTransactionService.record()` 前置预检 `available >= |qty|`，不足返回 `InsufficientStock`（HTTP 422）；`Adjustment`（盘点调账）不预检，由 `stock_ledger.upsert` 后置硬阻断兜底。
- 一库位一产品：`InventoryTransactionService.record()` 入库（qty>0）前置校验——目标 bin 若已有**其他产品** quantity>0 的台账（`StockLedgerRepo.find_other_occupant_in_bin`），返回 `BusinessRule`（HTTP 422）；目标产品自身或库位为空/归零时放行。覆盖所有入库路径（采购/生产/委外/调拨入库侧/形态转换/盘点/退料），出库类不受影响。
- 安全库存预警：消耗型扣减后 best-effort 触发 `LowStockAlertService.check_and_record`（`SUM(quantity) < SUM(safety_stock)` 时记录 + 发 `LowStockAlert` 事件）。

#### DeadLetterService — 死信队列

```rust
trait DeadLetterService {
    fn list_dead_letters(ctx, page, page_size) -> PaginatedResult<DomainEvent>;
    fn retry_one(ctx, event_id) -> Result<()>;
    fn archive(ctx, before: DateTime) -> Result<u64>;
}
```

#### PaginatedResult — 分页

```rust
struct PaginatedResult<T> {
    items: Vec<T>,
    total: u64,
    page: u32,
    page_size: u32,
    total_pages: u32,
}
```

所有查询接口统一返回 `PaginatedResult<T>`，不返回裸 `Vec<T>`。

#### DomainError — 统一错误模型

```rust
enum DomainError {
    NotFound(String),
    Duplicate(String),
    PermissionDenied(String),
    BusinessRule(String),
    Validation(String),
    ConcurrentConflict,
    Internal(#[from] anyhow::Error),  // 兼容老代码，gRPC 层记录完整 source 链
}
```

`ServiceError` 标记 `#[deprecated]`，新代码统一使用 `DomainError`。

### 业务集成规则

| 场景 | 必须调用 |
|------|----------|
| 创建单据 | `DocumentSequenceService.next_number()` 生成编号 |
| 单据状态变更 | `StateMachineService.transition()` + `get_allowed_transitions().contains()` 校验 |
| 业务操作产生事件 | `DomainEventBus.publish(EventPublishRequest{...})` |
| 数据变更（create/update/delete）| `AuditLogService.record(action: AuditAction, ...)`（同事务内）|
| 单据间产生关联 | `DocumentLinkService.create_link()` 或 `create_links()` |
| 涉及库存占用/释放 | `InventoryReservationService.reserve()` / `fulfill()` / `cancel()` |
| 产生成本 | `CostEntryService.create()` 或 `create_entries()` |
| 查询列表 | 返回 `PaginatedResult<T>`，不返回裸 `Vec<T>` |

## 接口统计

| 模块 | Service trait 数量 | 核心方法数 |
|------|-------------------|-----------|
| Shared | 15 | 36 |
| Sales CRM | 5 | 41 |
| Purchase SRM | 6 | 20 |
| WMS | 10 | 30 |
| MES | 6 | 29 |
| Outsourcing OM | 2 | 9 |
| Quality QMS | 4 | 16 |
| Financial FMS | 4 | 15 |
| Workflow V2 | 3 | 29 |
| Master Data | 13 | 71 |
| **合计** | **68** | **296** |

> **v5 新增事件（DomainEventType 扩展）**：RoutingCreated, RoutingUpdated, RoutingDeleted, BomRoutingChanged, LaborProcessDictCreated, LaborProcessDictUpdated, LaborProcessDictDeleted（原 19 → 26 种）
