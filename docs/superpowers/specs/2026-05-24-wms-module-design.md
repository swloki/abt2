---
name: wms-module
description: WMS 仓储模块 abt-core 完整实现规格
---

# WMS 仓储模块 — abt-core 实现规格

> 基于 `docs/uml-design/03-wms.html` 类图设计，完整实现仓储模块。

## 范围

在 `abt-core/src/wms/` 下完整实现 WMS 模块，包括：
- 数据库 Migration（22 张表）
- Model / Repo / Service Trait / Service Impl（11 个子模块）
- 共享基础设施集成（文档编号、状态机、事件总线、审计日志、库存预留、成本记录）
- 跨模块依赖用 stub 预留（QMS 检验门、MES 工单）
- 核心业务逻辑单测

不包含：Proto 定义、gRPC Handler、前端。

## 数据库：abt_v2

所有表建在 abt_v2 数据库中（`ABT_CORE_DATABASE_URL`）。

## 目录结构

```
wms/
  mod.rs
  warehouse/            (Warehouse + Zone + Bin)
  strategy/             (PutawayStrategy + PickStrategy)
  stock_ledger/         (库存台账查询)
  arrival_notice/       (来料通知)
  inventory_transaction/ (库存事务)
  material_requisition/ (领料)
  backflush/            (倒冲)
  cycle_count/          (盘点)
  transfer/             (调拨)
  form_conversion/      (形态转换)
  inventory_lock/       (锁库)
```

每个子目录：
- `mod.rs` — 模块声明 + pub use
- `model.rs` — 实体 + 请求/过滤结构体
- `repo.rs` — sqlx 原始 SQL
- `service.rs` — async_trait 接口
- `implt/mod.rs` — 具体实现

## 数据库 Schema

### 枚举类型

```sql
CREATE TYPE warehouse_type AS ENUM ('raw_material', 'finished_goods', 'semi_finished', 'consumable', 'virtual_outsource');
CREATE TYPE warehouse_status AS ENUM ('active', 'inactive');
CREATE TYPE zone_type AS ENUM ('receiving', 'storage', 'picking', 'packing', 'inspection', 'returns');
CREATE TYPE bin_status AS ENUM ('empty', 'occupied', 'locked', 'disabled');
CREATE TYPE arrival_status AS ENUM ('draft', 'received', 'inspecting', 'accepted', 'partially_accepted', 'rejected', 'cancelled');
CREATE TYPE transaction_type AS ENUM ('purchase_receipt', 'production_receipt', 'sales_shipment', 'material_issue', 'material_return', 'backflush', 'transfer', 'form_conversion', 'adjustment', 'lock', 'unlock', 'scrap');
CREATE TYPE requisition_status AS ENUM ('draft', 'confirmed', 'issued', 'cancelled');
CREATE TYPE backflush_status AS ENUM ('draft', 'executed', 'adjusted');
CREATE TYPE cycle_count_status AS ENUM ('draft', 'counting', 'completed', 'adjusted', 'cancelled');
CREATE TYPE transfer_status AS ENUM ('draft', 'in_transit', 'completed', 'cancelled');
CREATE TYPE conversion_dir AS ENUM ('consume', 'produce');
CREATE TYPE conversion_status AS ENUM ('draft', 'completed', 'cancelled');
CREATE TYPE lock_status AS ENUM ('active', 'released', 'cancelled');
CREATE TYPE putaway_type AS ENUM ('same_merge', 'nearest', 'fixed_bin', 'empty_first');
CREATE TYPE pick_type AS ENUM ('fifo', 'fefo', 'shortest_path', 'full_pallet');
```

### 表定义

#### warehouses

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| code | VARCHAR(50) | UNIQUE NOT NULL |
| name | VARCHAR(200) | NOT NULL |
| warehouse_type | warehouse_type | NOT NULL |
| status | warehouse_status | NOT NULL DEFAULT 'active' |
| address | TEXT | |
| manager_id | BIGINT | |
| is_virtual | BOOLEAN | NOT NULL DEFAULT false |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

索引：`idx_warehouses_type(warehouse_type)`, `idx_warehouses_deleted_at WHERE deleted_at IS NULL`

#### zones

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| code | VARCHAR(50) | NOT NULL |
| name | VARCHAR(200) | NOT NULL |
| zone_type | zone_type | NOT NULL |
| sort_order | INT | NOT NULL DEFAULT 0 |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

索引：`UNIQUE(warehouse_id, code) WHERE deleted_at IS NULL`

#### bins

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| zone_id | BIGINT | NOT NULL REFERENCES zones(id) |
| code | VARCHAR(50) | NOT NULL |
| name | VARCHAR(200) | NOT NULL |
| row_no | VARCHAR(20) | |
| column_no | VARCHAR(20) | |
| layer_no | VARCHAR(20) | |
| capacity_limit | DECIMAL(10,6) | |
| status | bin_status | NOT NULL DEFAULT 'empty' |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

索引：`UNIQUE(zone_id, code) WHERE deleted_at IS NULL`

#### putaway_strategies

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| name | VARCHAR(200) | NOT NULL |
| strategy_type | putaway_type | NOT NULL |
| warehouse_id | BIGINT | REFERENCES warehouses(id) |
| priority | INT | NOT NULL DEFAULT 0 |
| is_active | BOOLEAN | NOT NULL DEFAULT true |

#### pick_strategies

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| name | VARCHAR(200) | NOT NULL |
| strategy_type | pick_type | NOT NULL |
| warehouse_id | BIGINT | REFERENCES warehouses(id) |
| priority | INT | NOT NULL DEFAULT 0 |
| is_active | BOOLEAN | NOT NULL DEFAULT true |

#### stock_ledger

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| product_id | BIGINT | NOT NULL |
| warehouse_id | BIGINT | NOT NULL |
| zone_id | BIGINT | NOT NULL |
| bin_id | BIGINT | NOT NULL |
| batch_no | VARCHAR(50) | |
| quantity | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| reserved_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| available_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| unit_cost | DECIMAL(10,6) | |
| received_date | DATE | |
| expiry_date | DATE | |
| safety_stock | DECIMAL(18,6) | NOT NULL DEFAULT 0 |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

索引：`UNIQUE(product_id, warehouse_id, zone_id, bin_id, COALESCE(batch_no, ''))`, `idx_stock_product(product_id)`, `idx_stock_warehouse(warehouse_id)`

#### arrival_notices

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| purchase_order_id | BIGINT | |
| supplier_id | BIGINT | NOT NULL |
| arrival_date | DATE | NOT NULL |
| status | arrival_status | NOT NULL DEFAULT 'draft' |
| warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| zone_id | BIGINT | REFERENCES zones(id) |
| delivery_note | TEXT | |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

索引：`idx_arrival_status(status)`, `idx_arrival_supplier(supplier_id)`

#### arrival_notice_items

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| notice_id | BIGINT | NOT NULL REFERENCES arrival_notices(id) |
| order_item_id | BIGINT | |
| product_id | BIGINT | NOT NULL |
| declared_qty | DECIMAL(10,6) | NOT NULL |
| received_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| accepted_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| batch_no | VARCHAR(50) | |

#### inventory_transactions

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | |
| transaction_type | transaction_type | NOT NULL |
| product_id | BIGINT | NOT NULL |
| warehouse_id | BIGINT | NOT NULL |
| zone_id | BIGINT | |
| bin_id | BIGINT | |
| batch_no | VARCHAR(50) | |
| quantity | DECIMAL(10,6) | NOT NULL |
| unit_cost | DECIMAL(10,6) | |
| source_type | VARCHAR(50) | NOT NULL |
| source_id | BIGINT | NOT NULL |
| remark | TEXT | |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

索引：`idx_txn_product(product_id)`, `idx_txn_source(source_type, source_id)`, `idx_txn_type(transaction_type)`, `idx_txn_created(created_at)`

#### material_requisitions

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| work_order_id | BIGINT | NOT NULL |
| requisition_date | DATE | NOT NULL |
| status | requisition_status | NOT NULL DEFAULT 'draft' |
| warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| deleted_at | TIMESTAMPTZ | |

#### material_requisition_items

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| requisition_id | BIGINT | NOT NULL REFERENCES material_requisitions(id) |
| product_id | BIGINT | NOT NULL |
| requested_qty | DECIMAL(10,6) | NOT NULL |
| issued_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| variance_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| bin_id | BIGINT | |

#### backflush_records

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| work_order_id | BIGINT | NOT NULL |
| product_id | BIGINT | NOT NULL |
| completed_qty | DECIMAL(10,6) | NOT NULL |
| backflush_date | DATE | NOT NULL |
| status | backflush_status | NOT NULL DEFAULT 'draft' |
| variance_threshold | DECIMAL(10,6) | NOT NULL DEFAULT 0.05 |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

#### backflush_items

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| record_id | BIGINT | NOT NULL REFERENCES backflush_records(id) |
| component_id | BIGINT | NOT NULL |
| theoretical_qty | DECIMAL(10,6) | NOT NULL |
| actual_qty | DECIMAL(10,6) | NOT NULL |
| variance_qty | DECIMAL(10,6) | NOT NULL |
| variance_rate | DECIMAL(10,6) | NOT NULL |
| is_over_threshold | BOOLEAN | NOT NULL DEFAULT false |

#### cycle_counts

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| zone_id | BIGINT | REFERENCES zones(id) |
| count_date | DATE | NOT NULL |
| status | cycle_count_status | NOT NULL DEFAULT 'draft' |
| is_blind | BOOLEAN | NOT NULL DEFAULT false |
| remark | TEXT | |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

#### cycle_count_items

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| count_id | BIGINT | NOT NULL REFERENCES cycle_counts(id) |
| bin_id | BIGINT | NOT NULL |
| product_id | BIGINT | NOT NULL |
| batch_no | VARCHAR(50) | |
| system_qty | DECIMAL(10,6) | NOT NULL |
| counted_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| variance_qty | DECIMAL(10,6) | NOT NULL DEFAULT 0 |
| variance_reason | TEXT | |
| is_adjusted | BOOLEAN | NOT NULL DEFAULT false |

#### inventory_transfers

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| from_warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| from_zone_id | BIGINT | REFERENCES zones(id) |
| from_bin_id | BIGINT | REFERENCES bins(id) |
| to_warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| to_zone_id | BIGINT | REFERENCES zones(id) |
| to_bin_id | BIGINT | REFERENCES bins(id) |
| transfer_date | DATE | NOT NULL |
| status | transfer_status | NOT NULL DEFAULT 'draft' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

#### transfer_items

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| transfer_id | BIGINT | NOT NULL REFERENCES inventory_transfers(id) |
| product_id | BIGINT | NOT NULL |
| quantity | DECIMAL(10,6) | NOT NULL |
| batch_no | VARCHAR(50) | |

#### form_conversions

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| conversion_date | DATE | NOT NULL |
| status | conversion_status | NOT NULL DEFAULT 'draft' |
| remark | TEXT | NOT NULL DEFAULT '' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

#### conversion_items

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| conversion_id | BIGINT | NOT NULL REFERENCES form_conversions(id) |
| direction | conversion_dir | NOT NULL |
| product_id | BIGINT | NOT NULL |
| quantity | DECIMAL(10,6) | NOT NULL |
| unit_cost | DECIMAL(10,6) | NOT NULL |
| batch_no | VARCHAR(50) | |

#### inventory_locks

| 列 | 类型 | 约束 |
|----|------|------|
| id | BIGSERIAL | PK |
| doc_number | VARCHAR(50) | UNIQUE NOT NULL |
| product_id | BIGINT | NOT NULL |
| warehouse_id | BIGINT | NOT NULL REFERENCES warehouses(id) |
| locked_qty | DECIMAL(10,6) | NOT NULL |
| lock_reason | TEXT | NOT NULL |
| customer_id | BIGINT | |
| status | lock_status | NOT NULL DEFAULT 'active' |
| operator_id | BIGINT | NOT NULL |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT now() |

## Service 接口

### WarehouseService

```rust
#[async_trait]
pub trait WarehouseService: Send + Sync {
    async fn create(ctx, req: CreateWarehouseReq) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<Warehouse>;
    async fn list(ctx, filter: WarehouseFilter, page: u32, page_size: u32) -> Result<PaginatedResult<Warehouse>>;
    async fn update(ctx, id: i64, req: UpdateWarehouseReq) -> Result<()>;
    async fn delete(ctx, id: i64) -> Result<()>;
    async fn create_zone(ctx, warehouse_id: i64, req: CreateZoneReq) -> Result<i64>;
    async fn list_zones(ctx, warehouse_id: i64) -> Result<Vec<Zone>>;
    async fn create_bin(ctx, zone_id: i64, req: CreateBinReq) -> Result<i64>;
    async fn list_bins(ctx, zone_id: i64, filter: Option<BinFilter>) -> Result<PaginatedResult<Bin>>;
}
```

### StrategyService

```rust
#[async_trait]
pub trait StrategyService: Send + Sync {
    // Putaway
    async fn create_putaway(ctx, req: CreateStrategyReq) -> Result<i64>;
    async fn list_putaway(ctx, warehouse_id: Option<i64>) -> Result<Vec<PutawayStrategy>>;
    // Pick
    async fn create_pick(ctx, req: CreateStrategyReq) -> Result<i64>;
    async fn list_pick(ctx, warehouse_id: Option<i64>) -> Result<Vec<PickStrategy>>;
}
```

### StockLedgerService

```rust
#[async_trait]
pub trait StockLedgerService: Send + Sync {
    async fn query(ctx, filter: StockFilter, page: u32, page_size: u32) -> Result<PaginatedResult<StockLedger>>;
    async fn query_available(ctx, product_id: i64, warehouse_id: Option<i64>) -> Result<Decimal>;
    async fn upsert(ctx, product_id: i64, warehouse_id: i64, zone_id: i64, bin_id: i64, batch_no: Option<&str>, qty_delta: Decimal, cost: Option<Decimal>) -> Result<()>;
}
```

### ArrivalNoticeService

```rust
#[async_trait]
pub trait ArrivalNoticeService: Send + Sync {
    async fn create(ctx, req: CreateArrivalNoticeReq) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<ArrivalNotice>;
    async fn list(ctx, filter: ArrivalNoticeFilter, page: u32, page_size: u32) -> Result<PaginatedResult<ArrivalNotice>>;
    async fn receive(ctx, req: ReceiveArrivalNoticeReq) -> Result<()>;
    async fn inspect(ctx, req: InspectArrivalNoticeReq) -> Result<()>;
    async fn cancel(ctx, id: i64) -> Result<()>;
}
```

状态机：Draft → Received → Inspecting → Accepted/PartiallyAccepted/Rejected, Cancelled(仅 Draft)

### InventoryTransactionService

```rust
#[async_trait]
pub trait InventoryTransactionService: Send + Sync {
    async fn record(ctx, req: RecordTransactionReq) -> Result<i64>;
    async fn find_by_source(ctx, source_type: &str, source_id: i64) -> Result<Vec<InventoryTransaction>>;
    async fn query(ctx, filter: TransactionFilter, page: u32, page_size: u32) -> Result<PaginatedResult<InventoryTransaction>>;
}
```

Append-only，永不修改/删除。记录时自动更新 StockLedger。

### MaterialRequisitionService

```rust
#[async_trait]
pub trait MaterialRequisitionService: Send + Sync {
    async fn create_for_work_order(ctx, work_order_id: i64) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<MaterialRequisition>;
    async fn list(ctx, filter: RequisitionFilter, page: u32, page_size: u32) -> Result<PaginatedResult<MaterialRequisition>>;
    async fn confirm(ctx, id: i64) -> Result<()>;
    async fn issue(ctx, req: IssueMaterialReq) -> Result<()>;
    async fn cancel(ctx, id: i64) -> Result<()>;
}
```

状态机：Draft → Confirmed → Issued, Cancelled(Draft/Confirmed)

### BackflushService

```rust
#[async_trait]
pub trait BackflushService: Send + Sync {
    async fn execute(ctx, work_order_id: i64, completed_qty: Decimal) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<BackflushRecord>;
    async fn list(ctx, filter: BackflushFilter, page: u32, page_size: u32) -> Result<PaginatedResult<BackflushRecord>>;
}
```

状态机：Draft → Executed → Adjusted

### CycleCountService

```rust
#[async_trait]
pub trait CycleCountService: Send + Sync {
    async fn create(ctx, req: CreateCycleCountReq) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<CycleCount>;
    async fn list(ctx, filter: CycleCountFilter, page: u32, page_size: u32) -> Result<PaginatedResult<CycleCount>>;
    async fn start_count(ctx, id: i64) -> Result<()>;
    async fn count(ctx, req: CountCycleCountReq) -> Result<()>;
    async fn complete(ctx, id: i64) -> Result<()>;
    async fn adjust(ctx, id: i64) -> Result<()>;
    async fn cancel(ctx, id: i64) -> Result<()>;
}
```

状态机：Draft → Counting → Completed → Adjusted, Cancelled(Draft/Counting)

### TransferService

```rust
#[async_trait]
pub trait TransferService: Send + Sync {
    async fn create(ctx, req: CreateTransferReq) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<InventoryTransfer>;
    async fn list(ctx, filter: TransferFilter, page: u32, page_size: u32) -> Result<PaginatedResult<InventoryTransfer>>;
    async fn dispatch(ctx, id: i64) -> Result<()>;
    async fn complete(ctx, id: i64) -> Result<()>;
    async fn cancel(ctx, id: i64) -> Result<()>;
}
```

状态机：Draft → InTransit → Completed, Cancelled(仅 Draft)

### FormConversionService

```rust
#[async_trait]
pub trait FormConversionService: Send + Sync {
    async fn create(ctx, req: CreateConversionReq) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<FormConversion>;
    async fn list(ctx, filter: ConversionFilter, page: u32, page_size: u32) -> Result<PaginatedResult<FormConversion>>;
    async fn complete(ctx, id: i64) -> Result<()>;
    async fn cancel(ctx, id: i64) -> Result<()>;
}
```

状态机：Draft → Completed, Cancelled(仅 Draft)

### InventoryLockService

```rust
#[async_trait]
pub trait InventoryLockService: Send + Sync {
    async fn create(ctx, req: CreateLockReq) -> Result<i64>;
    async fn get(ctx, id: i64) -> Result<InventoryLock>;
    async fn list(ctx, filter: LockFilter, page: u32, page_size: u32) -> Result<PaginatedResult<InventoryLock>>;
    async fn release(ctx, id: i64) -> Result<()>;
    async fn cancel(ctx, id: i64) -> Result<()>;
}
```

状态机：Active → Released, Cancelled(仅 Active)

## 共享基础设施集成

| 业务操作 | 集成的共享服务 |
|----------|--------------|
| 创建单据（来料通知、领料单等） | DocumentSequenceService 生成编号 |
| 单据状态变更 | StateMachineService 校验转换合法性 |
| 数据变更 | AuditLogService 记录审计日志（同事务） |
| 业务事件完成 | DomainEventBus.publish() |
| 库存事务记录 | 自动更新 StockLedger |
| 领料/锁库占用库存 | InventoryReservationService |
| 成本相关事务 | CostEntryService（独立事务） |
| 单据间关联 | DocumentLinkService |

## 跨模块 Stub

以下跨模块依赖用 trait + stub 实现，待依赖模块就绪后替换：

| 依赖点 | Stub 接口 |
|--------|----------|
| 来料检验门（IQC） | `QualityGateStub::is_passed(ctx, source_type, source_id) -> bool`（默认 true） |
| MES 工单查询 | `WorkOrderStub::get_bom_components(ctx, work_order_id) -> Vec<BomComponent>`（返回空） |
| 产品信息查询 | `ProductStub::get(ctx, product_id) -> ProductInfo`（返回空） |

Stub trait 定义在 `wms/stubs.rs` 中，impl 在同文件。

## 事务模式

| 操作 | 模式 | 说明 |
|------|------|------|
| 库存预留、状态变更 | 同步强一致 | 失败回滚主事务 |
| 成本记录 | 独立事务 | 主事务提交后新开事务 |
| 审计日志 | 同事务 | 与业务操作同事务 |
| 事件发布 | Outbox | 写 outbox 表，后台消费 |

## 测试策略

核心业务逻辑单测，重点覆盖：
- StockLedger upsert 的并发安全（quantity 累加/扣减、不可为负）
- 状态机转换合法性（非法转换返回 DomainError）
- 倒冲差异计算（theoretical vs actual、阈值判断）
- 盘点差异计算与自动调整
- 库存事务记录后 StockLedger 自动同步
