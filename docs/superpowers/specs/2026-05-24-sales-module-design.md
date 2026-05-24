# 销售模块 (CRM) 实现设计

> 基于 `docs/uml-design/01-sales.html` 设计文档，严格按规范实现

## 实现范围

分阶段实现 5 个子模块，按业务流转顺序：

| 阶段 | 子模块 | DocumentType | 前缀 | 依赖 |
|------|--------|-------------|------|------|
| 1 | Quotation 报价单 | Quotation=1 | QUO | MasterData.CustomerService |
| 2 | SalesOrder 销售订单 | SalesOrder=2 | SO | QuotationService |
| 3 | ShippingRequest 发货申请 | ShippingRequest=3 | SR | SalesOrderService |
| 4 | SalesReturn 销售退货 | SalesReturn=4 | SRT | ShippingRequestService |
| 5 | Reconciliation 月对账 | Reconciliation=5 | REC | ShippingRequestService |

每个子模块内部按分层顺序：Migration → Model → Repo → Service Trait → Service Impl。

## 阶段 1：Quotation 报价单

### 数据库 Migration

文件：`abt-core/migrations/002_create_sales_quotation.sql`

```sql
-- quotations 主表
CREATE TABLE quotations (
    id           BIGSERIAL PRIMARY KEY,
    doc_number   VARCHAR(30) NOT NULL UNIQUE,
    customer_id  BIGINT NOT NULL,
    contact_id   BIGINT NOT NULL,
    sales_rep_id BIGINT NOT NULL,
    quotation_date DATE NOT NULL DEFAULT CURRENT_DATE,
    valid_until  DATE NOT NULL,
    status       SMALLINT NOT NULL DEFAULT 1,
    total_amount DECIMAL(20,4) NOT NULL DEFAULT 0,
    total_cost   DECIMAL(20,4) NOT NULL DEFAULT 0,
    estimated_margin DECIMAL(5,2) NOT NULL DEFAULT 0,
    payment_terms VARCHAR(100) NOT NULL DEFAULT '',
    delivery_terms VARCHAR(100) NOT NULL DEFAULT '',
    remark       TEXT NOT NULL DEFAULT '',
    operator_id  BIGINT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at   TIMESTAMPTZ
);

CREATE INDEX idx_quotations_customer ON quotations(customer_id);
CREATE INDEX idx_quotations_status ON quotations(status);
CREATE INDEX idx_quotations_doc_number ON quotations(doc_number);

-- quotation_items 明细表
CREATE TABLE quotation_items (
    id            BIGSERIAL PRIMARY KEY,
    quotation_id  BIGINT NOT NULL REFERENCES quotations(id),
    line_no       INT NOT NULL,
    product_id    BIGINT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    quantity      DECIMAL(18,6) NOT NULL,
    unit          VARCHAR(20) NOT NULL DEFAULT '',
    unit_price    DECIMAL(18,6) NOT NULL,
    unit_cost     DECIMAL(18,6) NOT NULL DEFAULT 0,
    discount_rate DECIMAL(5,2) NOT NULL DEFAULT 0,
    amount        DECIMAL(20,4) NOT NULL,
    delivery_date DATE
);

CREATE INDEX idx_quotation_items_quotation ON quotation_items(quotation_id);
```

状态值：1=Draft, 2=Sent, 3=Accepted, 4=Rejected, 5=Expired

### Model 层

文件：`abt-core/src/sales/quotation/model.rs`

- `Quotation` struct — `#[derive(Debug, Clone, sqlx::FromRow)]`，字段与表对应
- `QuotationItem` struct — `#[derive(Debug, Clone, sqlx::FromRow)]`
- `QuotationStatus` enum — `#[repr(i16)]`，含 sqlx Type/Encode/Decode + serde
- `CreateQuotationReq` — customer_id, contact_id, valid_until, items: Vec<CreateQuotationItemReq>, payment_terms, delivery_terms, remark
- `CreateQuotationItemReq` — product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, delivery_date
- `UpdateQuotationReq` — 所有字段 Option，用于部分更新
- `QuotationQuery` — customer_id, status, date_from, date_to, keyword

### Repo 层

文件：`abt-core/src/sales/quotation/repo.rs`

`QuotationRepo`：
- `create(executor, doc_number, req, operator_id) -> Result<i64>`
- `find_by_id(executor, id) -> Result<Option<Quotation>>`
- `find_by_doc_number(executor, doc_number) -> Result<Option<Quotation>>`
- `update(executor, id, req) -> Result<()>`
- `update_status(executor, id, status) -> Result<()>`
- `update_amounts(executor, id, total_amount, total_cost, margin) -> Result<()>`
- `expire_overdue(executor) -> Result<i64>` — 批量过期
- `query(executor, filter, page, data_scope, operator_id, department_id) -> Result<PaginatedResult<Quotation>>`

`QuotationItemRepo`：
- `create_batch(executor, quotation_id, items) -> Result<()>`
- `find_by_quotation_id(executor, quotation_id) -> Result<Vec<QuotationItem>>`
- `delete_by_quotation_id(executor, quotation_id) -> Result<()>`

### Service Trait

文件：`abt-core/src/sales/quotation/service.rs`

严格按照设计文档 01-sales.html 的 QuotationService 接口：

```rust
#[async_trait]
pub trait QuotationService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateQuotationReq) -> Result<i64, DomainError>;
    async fn find_by_id(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Quotation, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateQuotationReq) -> Result<(), DomainError>;
    async fn submit(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn accept(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn reject(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn expire(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn batch_expire_overdue(&self, ctx: ServiceContext<'_>) -> Result<i32, DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: QuotationQuery, page: PageParams) -> Result<PaginatedResult<Quotation>, DomainError>;
}
```

### Service Impl

文件：`abt-core/src/sales/quotation/implt/mod.rs`

`QuotationServiceImpl` 构造函数注入：

```rust
pub struct QuotationServiceImpl {
    repo: QuotationRepo,
    item_repo: QuotationItemRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    customer_svc: Arc<dyn CustomerService>,
}
```

#### create 流程

1. `customer_svc.validate_contact_ownership(ctx, customer_id, contact_id)` 校验
2. `doc_seq.next_number(ctx, DocumentType::Quotation)` 生成编号
3. 计算 `total_amount`, `total_cost`, `estimated_margin`（遍历 items）
4. `repo.create()` 插入主表
5. `item_repo.create_batch()` 批量插入明细
6. `state_machine.transition(ctx, "QuotationStatus", id, "Draft", None)` 初始化状态
7. `audit.record(ctx, "Quotation", id, AuditAction::Create, ...)`
8. `event_bus.publish(...)` — 发布 QuotationCreated 事件
9. 返回 id

#### update 流程

1. `repo.find_by_id()` 获取现有记录
2. 校验 status == Draft（否则 BusinessRule 错误）
3. 如果更新了 valid_until，校验 > 当前日期
4. 如果更新了 items：先 `item_repo.delete_by_quotation_id()` 再 `create_batch()`
5. 重算金额 → `repo.update_amounts()`
6. `repo.update()` 更新主表
7. `audit.record(ctx, "Quotation", id, AuditAction::Update, ...)`

#### submit 流程

1. `repo.find_by_id()` → 校验 status == Draft（状态机校验）
2. `item_repo.find_by_quotation_id()` → 校验 items 非空
3. `state_machine.transition(ctx, "QuotationStatus", id, "Sent", None)`
4. `repo.update_status(id, Sent)`
5. `audit.record(ctx, "Quotation", id, AuditAction::Transition, ...)`
6. `event_bus.publish(...)` — QuotationSubmitted

#### accept / reject 流程

1. 状态机校验 Sent → Accepted/Rejected
2. `repo.update_status()`
3. `audit.record()`
4. `event_bus.publish()`

#### expire 流程

1. 状态机校验 Draft/Sent → Expired
2. `repo.update_status()`
3. `audit.record()`

#### batch_expire_overdue 流程

1. `repo.expire_overdue()` — 批量更新 valid_until < now 且 status == Sent 的记录
2. 返回影响行数

#### list 流程

1. `repo.query()` 返回 PaginatedResult

## 阶段 2：SalesOrder 销售订单

（详细设计与阶段 1 同构，关键差异点如下）

### 额外依赖

- `QuotationService` — `create_from_quotation` 需要查询报价单
- `DocumentLinkService` — 记录 DERIVED_FROM 关联
- `InventoryReservationService` — confirm 时做 Soft 预留（TTL=7d）

### 独有方法

- `create_from_quotation(ctx, quotation_id)` — 从报价单派生
  - 校验报价单 status == Accepted 且 valid_until 未过期
  - 复制 items、customer_id、contact_id
  - sales_rep_id 从报价单继承
  - `doc_link.create_link(SalesOrder, Quotation, DerivedFrom)`
- `confirm(ctx, id)` — 触发 `InventoryReservation.reserve(Soft, TTL=7d)`
- `start_progress(ctx, id)` — Confirmed → InProduction
- `complete(ctx, id)` — 校验所有行 delivered_qty >= quantity
- `cancel(ctx, id)` — Draft/Confirmed → Cancelled，释放预留

### 状态矩阵

Draft → Confirmed → InProduction → Completed
Draft/Confirmed → Cancelled

## 阶段 3：ShippingRequest 发货申请

### 额外依赖

- `SalesOrderService` — 查询订单信息
- `DocumentLinkService` — TRIGGERS 关联
- `InventoryReservationService` — confirm 时释放预留、ship 时 fulfill
- `CostEntryService` — ship 时记录 COGS

### 独有方法

- `create_from_order(ctx, order_id, items)` — 从订单创建发货
  - 校验 order status >= Confirmed
  - 校验 requested_qty <= order_qty - shipped_qty
- `confirm(ctx, id)` — QMS OQC hard gate 检查
- `pick(ctx, id)` — Confirmed → Picking
- `ship(ctx, id)` — Picking → Shipped
  - 更新 order_item.shipped_qty
  - `inv_res.fulfill()`
  - `cost_entry.create_entries()` (COGS)
  - `event_bus.publish(ShipmentShipped)`
- `cancel(ctx, id)` — Draft/Confirmed → Cancelled

## 阶段 4：SalesReturn 销售退货

### 额外依赖

- `ShippingRequestService` — 查询发货信息
- `DocumentLinkService` — REFERENCES 关联
- `CostEntryService` — complete 时冲减

### 独有方法

- `create(ctx, req)` — 校验发货单 status == Shipped，return_qty <= shipped_qty - returned_qty
- `approve(ctx, id)` — Draft → Confirmed
- `receive(ctx, id)` — Confirmed → Received
- `inspect(ctx, id)` — Received → Inspecting，触发 QMS 质检
- `complete(ctx, id)` — Inspecting → Completed
  - 更新 order_item.returned_qty
  - 按 disposition 处理（Restock/Scrap/Rework）
  - `cost_entry.create_entries()` (reverse)
- `reject(ctx, id)` — Draft → Rejected

## 阶段 5：Reconciliation 月对账

### 额外依赖

- `ShippingRequestService` — 查询发货明细
- `DocumentLinkService` — RECONCILES 关联
- `CostEntryService` — confirm 时生成应收账款凭证

### 独有方法

- `create(ctx, customer_id, period)` — 唯一约束 customer_id + period
  - 自动聚合该客户+期间内所有 Shipped 的发货明细
  - `doc_link.create_links()` 批量关联
- `send(ctx, id)` — Draft → Sent
- `confirm(ctx, id)` — Sent → Confirmed，校验所有 item.confirmed
- `dispute(ctx, id)` — Sent/Confirmed → Disputed
- `reopen(ctx, id)` — Disputed → Draft
- `force_settle(ctx, id)` — Disputed → Settled
- `settle(ctx, id)` — Confirmed → Settled
  - `cost_entry.create_entries()` (AR voucher)
  - FMS 集成：CashJournal + WriteOff

## 共享服务集成规则

每个子模块统一遵循以下集成模式（来自 README.md 业务集成规则）：

| 操作 | 集成 |
|------|------|
| 创建单据 | `DocumentSequenceService.next_number()` |
| 状态变更 | `StateMachineService.transition()` + `get_allowed_transitions()` |
| 业务操作完成 | `DomainEventBus.publish(EventPublishRequest{...})` |
| 数据变更 | `AuditLogService.record(action: AuditAction, ...)` 同事务内 |
| 单据关联 | `DocumentLinkService.create_link()` 或 `create_links()` |
| 库存操作 | `InventoryReservationService.reserve()` / `fulfill()` / `cancel()` |
| 成本记录 | `CostEntryService.create()` 或 `create_entries()` |
| 列表查询 | 返回 `PaginatedResult<T>` |

## 文件结构

```
abt-core/
  migrations/
    002_create_sales_quotation.sql
    003_create_sales_order.sql
    004_create_shipping_request.sql
    005_create_sales_return.sql
    006_create_reconciliation.sql
  src/sales/
    mod.rs                          # pub mod 声明
    quotation/
      mod.rs                        # pub use 重新导出
      model.rs                      # Quotation, QuotationItem, QuotationStatus, Req/Query
      repo.rs                       # QuotationRepo, QuotationItemRepo
      service.rs                    # QuotationService trait
      implt/mod.rs                  # QuotationServiceImpl
    sales_order/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
    shipping_request/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
    sales_return/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
    reconciliation/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
```
