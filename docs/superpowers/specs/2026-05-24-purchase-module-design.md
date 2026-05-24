# 采购模块 (SRM) 实现设计

> 2026-05-24 | 基于 `docs/uml-design/02-purchase.html` v2 设计审阅修订

## 范围

在 `abt-core` 中实现完整的采购模块，包含 6 个子实体、12 张数据库表、6 个 Service trait。不包含 gRPC handler 层。

## 模块结构

```
abt-core/src/purchase/
├── mod.rs                  # 模块声明 + pub use
├── enums.rs                # 7 个采购专属枚举
├── quotation/              # 采购报价
│   ├── mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
├── order/                  # 采购订单
│   ├── mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
├── return_order/           # 采购退货
│   ├── mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
├── reconciliation/         # 对账单
│   ├── mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
├── payment/                # 付款申请
│   ├── mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
└── misc_request/           # 零星请购
    ├── mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs
```

## 实现顺序

按依赖关系分层渐增：

1. **Migration** — 12 张表 + 状态机转换规则
2. **枚举 + Model** — Rust 数据结构（`#[repr(i16)]` + sqlx + serde）
3. **Repository** — sqlx SQL 查询层
4. **Service Trait** — 6 个 `#[async_trait]` 接口
5. **ServiceImpl** — 业务逻辑 + 共享层集成
6. **`lib.rs` 工厂函数** — 模块注册

## 枚举定义

全部 `#[repr(i16)]`，参考 `shared/enums` 编码模式（sqlx Type/Encode/Decode + serde）：

| 枚举 | 值 |
|------|------|
| `PurchaseQuotationStatus` | Draft=1, Active=2, Expired=3, Cancelled=4 |
| `PurchaseOrderStatus` | Draft=1, Confirmed=2, PartiallyReceived=3, Received=4, Closed=5, Cancelled=6 |
| `PurchaseReturnStatus` | Draft=1, Confirmed=2, Shipped=3, Settled=4, Cancelled=5 |
| `PurchaseReconStatus` | Draft=1, Confirmed=2, Settled=3 |
| `PaymentStatus` | Draft=1, Approved=2, Paid=3, Cancelled=4 |
| `PaymentMethod` | BankTransfer=1, Cash=2, Note=3 |
| `MiscRequestStatus` | Draft=1, Approved=2, Purchasing=3, Received=4, Closed=5, Cancelled=6 |

## 数据库表

### 主表（6 张）

**purchase_quotations** — 采购报价

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(32) | NOT NULL UNIQUE |
| supplier_id | BIGINT | NOT NULL FK→suppliers |
| quotation_date | DATE | NOT NULL |
| valid_from | DATE | NOT NULL |
| valid_until | DATE | NOT NULL |
| status | SMALLINT | NOT NULL DEFAULT 1 |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

**purchase_orders** — 采购订单

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(32) | NOT NULL UNIQUE |
| supplier_id | BIGINT | NOT NULL FK→suppliers |
| order_date | DATE | NOT NULL |
| expected_delivery_date | DATE | |
| status | SMALLINT | NOT NULL DEFAULT 1 |
| total_amount | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| payment_terms | TEXT | |
| delivery_address | TEXT | |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

**purchase_returns** — 采购退货

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(32) | NOT NULL UNIQUE |
| order_id | BIGINT | NOT NULL FK→purchase_orders |
| supplier_id | BIGINT | NOT NULL FK→suppliers |
| return_date | DATE | NOT NULL |
| status | SMALLINT | NOT NULL DEFAULT 1 |
| return_reason | TEXT | NOT NULL |
| total_amount | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

**purchase_reconciliations** — 对账单

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(32) | NOT NULL UNIQUE |
| supplier_id | BIGINT | NOT NULL FK→suppliers |
| period | VARCHAR(7) | NOT NULL |
| status | SMALLINT | NOT NULL DEFAULT 1 |
| total_amount | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| confirmed_amount | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| difference | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

约束：`UNIQUE(supplier_id, period) WHERE deleted_at IS NULL`

**payment_requests** — 付款申请

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(32) | NOT NULL UNIQUE |
| supplier_id | BIGINT | NOT NULL FK→suppliers |
| reconciliation_id | BIGINT | FK→purchase_reconciliations |
| payment_date | DATE | NOT NULL |
| amount | DECIMAL(20,4) | NOT NULL |
| status | SMALLINT | NOT NULL DEFAULT 1 |
| payment_method | SMALLINT | NOT NULL |
| bank_account_id | BIGINT | |
| invoice_number | VARCHAR(64) | |
| invoice_amount | DECIMAL(20,4) | |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

**miscellaneous_requests** — 零星请购

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(32) | NOT NULL UNIQUE |
| department_id | BIGINT | NOT NULL |
| request_date | DATE | NOT NULL |
| status | SMALLINT | NOT NULL DEFAULT 1 |
| total_amount | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| purpose | TEXT | NOT NULL |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

### 明细表（6 张）

**purchase_quotation_items**

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| quotation_id | BIGINT | NOT NULL FK→purchase_quotations |
| product_id | BIGINT | NOT NULL |
| line_no | INT | NOT NULL |
| unit_price | DECIMAL(18,6) | NOT NULL |
| min_order_qty | DECIMAL(18,6) | |
| lead_time_days | INT | |
| currency | VARCHAR(3) | NOT NULL DEFAULT 'CNY' |
| is_preferred | BOOLEAN | NOT NULL DEFAULT false |

**purchase_order_items**

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| order_id | BIGINT | NOT NULL FK→purchase_orders |
| line_no | INT | NOT NULL |
| product_id | BIGINT | NOT NULL |
| description | TEXT | NOT NULL DEFAULT '' |
| quantity | DECIMAL(18,6) | NOT NULL |
| unit_price | DECIMAL(18,6) | NOT NULL |
| amount | DECIMAL(20,4) | NOT NULL |
| received_qty | DECIMAL(18,6) | NOT NULL DEFAULT 0 |
| inspected_qty | DECIMAL(18,6) | NOT NULL DEFAULT 0 |
| returned_qty | DECIMAL(18,6) | NOT NULL DEFAULT 0 |
| quotation_item_id | BIGINT | FK→purchase_quotation_items |
| expected_delivery_date | DATE | |

**purchase_return_items**

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| return_id | BIGINT | NOT NULL FK→purchase_returns |
| order_item_id | BIGINT | NOT NULL FK→purchase_order_items |
| product_id | BIGINT | NOT NULL |
| returned_qty | DECIMAL(18,6) | NOT NULL |
| unit_price | DECIMAL(18,6) | NOT NULL |
| amount | DECIMAL(20,4) | NOT NULL |

**purchase_recon_items**

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| reconciliation_id | BIGINT | NOT NULL FK→purchase_reconciliations |
| order_id | BIGINT | NOT NULL |
| order_item_id | BIGINT | NOT NULL |
| received_qty | DECIMAL(18,6) | NOT NULL |
| returned_qty | DECIMAL(18,6) | NOT NULL DEFAULT 0 |
| returned_amount | DECIMAL(20,4) | NOT NULL DEFAULT 0 |
| unit_price | DECIMAL(18,6) | NOT NULL |
| amount | DECIMAL(20,4) | NOT NULL |
| confirmed | BOOLEAN | NOT NULL DEFAULT false |

**misc_request_items**

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| request_id | BIGINT | NOT NULL FK→miscellaneous_requests |
| line_no | INT | NOT NULL |
| item_name | TEXT | NOT NULL |
| specification | TEXT | |
| quantity | DECIMAL(18,6) | NOT NULL |
| unit | VARCHAR(16) | NOT NULL |
| estimated_price | DECIMAL(18,6) | |
| remark | TEXT | |

## Service Trait 接口

### PurchaseQuotationService

```rust
#[async_trait]
pub trait PurchaseQuotationService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreatePurchaseQuotationRequest) -> Result<i64, DomainError>;
    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseQuotation, DomainError>;
    async fn activate(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn list(ctx: ServiceContext<'_>, query: PurchaseQuotationQuery) -> Result<PaginatedResult<PurchaseQuotation>, DomainError>;
    async fn compare(ctx: ServiceContext<'_>, product_id: i64) -> Result<Vec<QuotationComparison>, DomainError>;
}
```

### PurchaseOrderService

```rust
#[async_trait]
pub trait PurchaseOrderService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreatePurchaseOrderRequest) -> Result<i64, DomainError>;
    async fn create_from_quotation(ctx: ServiceContext<'_>, quotation_id: i64) -> Result<i64, DomainError>;
    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseOrder, DomainError>;
    async fn confirm(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn list(ctx: ServiceContext<'_>, query: PurchaseOrderQuery) -> Result<PaginatedResult<PurchaseOrder>, DomainError>;
}
```

### PurchaseReturnService

```rust
#[async_trait]
pub trait PurchaseReturnService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreatePurchaseReturnRequest) -> Result<i64, DomainError>;
    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReturn, DomainError>;
    async fn confirm(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
```

### PurchaseReconciliationService

```rust
#[async_trait]
pub trait PurchaseReconciliationService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, supplier_id: i64, period: String) -> Result<i64, DomainError>;
    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReconciliation, DomainError>;
    async fn confirm(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
```

### PaymentRequestService

```rust
#[async_trait]
pub trait PaymentRequestService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreatePaymentRequestRequest) -> Result<i64, DomainError>;
    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PaymentRequest, DomainError>;
    async fn approve(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn mark_paid_by_fms(ctx: ServiceContext<'_>, id: i64, payment_doc_no: String) -> Result<(), DomainError>;
}
```

### MiscellaneousRequestService

```rust
#[async_trait]
pub trait MiscellaneousRequestService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreateMiscRequestRequest) -> Result<i64, DomainError>;
    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<MiscellaneousRequest, DomainError>;
    async fn approve(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
```

## 共享层集成

| 场景 | 调用的共享服务 |
|------|---------------|
| 创建单据 | `DocumentSequenceService.next_number()` |
| 单据状态变更 | `StateMachineService.transition()` + `get_allowed_transitions()` |
| 业务操作产生事件 | `DomainEventBus.publish()` |
| 数据变更 | `AuditLogService.record()` (同事务内) |
| 单据间关联 | `DocumentLinkService.create_link()` / `create_links()` |
| 涉及库存 | `InventoryReservationService` |
| 产生成本 | `CostEntryService` |
| 查询列表 | 返回 `PaginatedResult<T>` |

## 业务逻辑要点

### PurchaseOrderService.confirm()

1. 前置校验：`SupplierService.get()` 校验 status ∉ {Blacklisted, Disqualified}
2. 行级校验：所有 Item quantity > 0 且 unit_price > 0
3. 报价关联校验：若关联 Quotation，quotation.status == Active 且 valid_until > now
4. 状态机转换：Draft → Confirmed
5. 发布事件：PurchaseOrderConfirmed → Outbox → WMS 异步创建 ArrivalNotice
6. 审计日志

### PurchaseReconciliationService.confirm()

1. 汇总计算：应付总额 = ∑(收货金额) - ∑(退货金额) + 调整项
2. 更新 PurchaseReconItem 的 confirmed 标记
3. 驱动关联 PurchaseReturn 状态：Shipped → Settled
4. 发布事件通知 FMS

### PaymentRequestService

- **create**：三单匹配校验（PO.received_qty + Invoice qty/amount 匹配，tolerance_rate 可配置）
- **approve**：CostEntry(cash outflow)
- **mark_paid_by_fms**：FMS 回调或监听 PaymentExecuted 事件

### SRM / FMS 边界

- SRM 管"应付申请 + 三单匹配"
- FMS 管"资金流水 + 核销"
- PaymentRequest.mark_paid_by_fms() 是 FMS 执行付款后回调

### 并发控制

所有写操作（confirm, approve, update）使用 `WHERE id = ? AND updated_at = ?` 乐观锁，失败抛 `DomainError::ConcurrentConflict`。

### 幂等性

create, confirm, approve 等写接口支持 `idempotency_key`，依赖共享层 `IdempotencyService`。

### 行级数据权限

list 底层强制过滤 DataScope：
- `DataScope::Self` → `WHERE operator_id = ctx.operator_id`
- `DataScope::Department` → `WHERE department_id IN ctx.allowed_dept_ids`
