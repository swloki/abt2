# UML 类图设计文档

> 2026-05-22 设计，基于 target.md 蓝图

## 文件说明

### HTML 可视化预览（Mermaid + 缩放/拖拽）

| 文件 | 内容 | 实体数 |
|------|------|--------|
| [00-module-dependencies.html](00-module-dependencies.html) | 模块间接口依赖关系总览 | 9 模块 + 51 Service trait |
| [00-shared-infrastructure.html](00-shared-infrastructure.html) | 共享基础设施层 — 文档编号、文档关联、库存预留、成本账本、领域事件(Outbox)、状态机、审计日志、幂等去重 | 8 核心 + 9 枚举 |
| [01-sales.html](01-sales.html) | 销售模块 — 报价、订单、发货、退货、对账（DomainError + 业务校验规则 + 语义化状态方法） | 5 主表 + 5 明细表 |
| [02-purchase.html](02-purchase.html) | 采购模块 — 采购报价、订单、退货、对账、付款、零星请购（Supplier 已迁至 Master Data） | 6 主表 + 6 明细表 |
| [03-wms.html](03-wms.html) | 仓储模块 — 三级库位、策略引擎、来料、库存事务、领料、倒冲、盘点、调拨、形态转换、锁库 | 12 主表 + 10 明细表 |
| [04-mes.html](04-mes.html) | 生产模块 — 计划、工单、生产批次(流转卡)、工序、报工、计件工资、报检、完工入库（委外委托 OM） | 7 主表 + 3 明细表 + 3 枚举 |
| [05-outsourcing.html](05-outsourcing.html) | 委外管理 — 委外单、发料明细、追踪节点、转自制 | 3 主表 + 7 节点类型 |
| [06-qms.html](06-qms.html) | 质量管理 v2.3 — 检验规格、检验结果、MRB不良评审、RMA客诉（QualityGateService独立 + QualityGateStatus + Req/Filter + 乐观锁 + 工作流集成 + execute_disposition + InCallerTx硬门 + Guard Conditions + 幂等约束 + JSONB强类型） | 5 Service + 4 主表 + 10 枚举 + 8 Req + 4 Filter + 3 JSONB类型 |
| [07-fms.html](07-fms.html) | 财务管理 v2 — 日记账、日记账明细、核销、费用报销、成本核算（强类型 Filter + BalanceSummary + 幂等键 + TransactionMode + CounterpartyRef + 工作流解耦） | 5 主表 + 1 明细表 + 4 请求/返回结构体 |
| [08-workflow-engine.html](08-workflow-engine.html) | 工作流引擎 V2 — 依赖共享层事件/状态机，Saga 补偿 + 增强节点 | 3 Service + 4 核心实体 |
| [09-master-data.html](09-master-data.html) | 主数据模块 v4 — 产品目录、分类、价格、BOM、客户(Customer)、供应商(Supplier)（CQRS 拆分 + ServiceContext + 乐观锁 + 客商统一主数据） | 10 Service + 21 核心实体 + 10 请求结构体 |

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

trait AuditLogService {
    fn record(ctx, entity_type, entity_id, action: AuditAction, changes, context) -> Result<i64>;
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
- `reserve`: **ContinueOnError**（部分行库存不足不影响其他行）
- `create_entries`（CostEntry）: **Atomic**（双层记账必须完整）

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
| Shared | 9 | 20 |
| Sales CRM | 5 | 41 |
| Purchase SRM | 6 | 20 |
| WMS | 9 | 28 |
| MES | 6 | 29 |
| Outsourcing OM | 2 | 9 |
| Quality QMS | 4 | 16 |
| Financial FMS | 4 | 15 |
| Workflow V2 | 3 | 29 |
| Master Data | 10 | 49 |
| **合计** | **50** | **236** |
