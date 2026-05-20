---
title: "feat: Add Sales Order, Shipping, Return & Reconciliation Modules"
type: feat
status: active
date: 2026-05-20
origin: docs/superpowers/specs/2026-05-20-sales-order-shipping-return-reconciliation-design.md
---

# feat: Add Sales Order, Shipping, Return & Reconciliation Modules

## Summary

实现销售管理系统第二章剩余四个子模块：销售订单、发货申请、销售退货、月对账单。与已完成的报价单模块构成完整销售链路。四个模块通过订单行项目的 `shipped_qty`/`returned_qty` 字段实现分批发货和退货数量跟踪。发货出库和退货入库集成现有 `InventoryService`。

---

## Requirements

- R1. 销售订单 CRUD：独立创建或从已接受报价单转入，草稿可编辑/删除，确认后行项目锁定仅主信息可改
- R2. 销售订单状态流转：Draft→Confirmed→InProgress→Completed/Cancelled
- R3. 发货申请 CRUD：必须关联订单，选择行项目和发货数量，校验不超剩余可发量
- R4. 发货两步确认：Pending→Confirmed（销售确认）→Shipped（仓库出库扣库存）
- R5. 销售退货 CRUD：必须关联已出库的发货单，退货数量不超过发货量减已退量
- R6. 销售退货状态流转：Pending→Approved→Received→Completed/Rejected，Completed 时退货入库
- R7. 月对账单：按客户按月自动汇总发货+退货明细，支持手动调整项
- R8. 月对账单状态流转：Draft→Confirmed→Approved
- R9. 编号自动生成：SO/SR/RT/RC 前缀，复用 document_sequences 服务

---

## Scope Boundaries

- 不含客户主数据（客户名称为纯文本字段）
- 不含审批流（所有模块无审批环节）
- 不含库存预留/ATP 检查
- 库存集成限于发货出库和退货入库，不涉及库存预留层
- 不含报价-订单毛利分析
- 不含前端代码

---

## Context & Research

### Relevant Code and Patterns

- 已完成模块参考：`abt/src/implt/quotation_service_impl.rs`（ServiceError 使用、状态常量、事务内编号生成）
- Proto 模式：`proto/abt/v1/quotation.proto`（CRUD + 列表分页）
- Model 模式：`abt/src/models/quotation.rs`（FromRow、Vec<Items>、Query struct）
- Repository 模式：`abt/src/repositories/quotation_repo.rs`（Executor、build_fuzzy_pattern、分页查询）
- Handler 模式：`abt-grpc/src/handlers/quotation.rs`（事务管理、Model↔Proto 转换）
- 编号服务：`abt/src/repositories/document_sequence_repo.rs`（next_number, ensure_sequence）
- 工厂函数：`abt/src/lib.rs`（`get_*_service` 模式，返回 `impl Trait`）
- 服务注册：`abt-grpc/src/server.rs`（add_service with_interceptor）
- 库存集成：`abt/src/service/inventory_service.rs`（stock_in/stock_out 接受 `StockChangeRequest` 含 `ref_order_type`/`ref_order_id`）
- 分页工具：`abt/src/repositories/mod.rs`（PaginationParams、PaginatedResult、build_fuzzy_pattern）

---

## Key Technical Decisions

- **订单行项目跟踪字段**：`shipped_qty`/`returned_qty` 在 order_item 上直接累加，避免每次 JOIN 汇总
- **发货数量校验在 service 层**：创建发货申请时查询 order_items 的 shipped_qty，确保不超量
- **库存集成通过 InventoryService trait**：发货出库和退货入库调用现有 `stock_out`/`stock_in`，传入 `ref_order_type="shipping_request"/"sales_return"` + 单据 ID
- **对账单自动生成**：创建时查询该客户该月 Shipped 发货单和 Completed 退货单的行项目明细，插入 reconciliation_items，手动调整项单独接口添加
- **迁移编号从 047 开始**：045/046 已被报价单模块占用（在 feat/sales-quotation 分支上）

---

## Implementation Units

### U1. Database Migrations

**Goal:** 创建 8 张新表（sales_orders, sales_order_items, shipping_requests, shipping_request_items, sales_returns, sales_return_items, reconciliation_statements, reconciliation_items）及索引，初始化 SO/SR/RT/RC 序列记录

**Requirements:** R1, R3, R5, R7, R9

**Dependencies:** None

**Files:**
- Create: `abt/migrations/047_create_sales_orders.sql`
- Create: `abt/migrations/048_create_shipping_requests.sql`
- Create: `abt/migrations/049_create_sales_returns.sql`
- Create: `abt/migrations/050_create_reconciliation.sql`
- Create: `abt/migrations/051_seed_sales_sequences.sql`

**Approach:**
- 按依赖顺序创建：sales_orders → shipping_requests → sales_returns → reconciliation
- 每个 migration 一个文件，包含主表 + 行项目表 + 索引
- 051 为 document_sequences 插入 SO/SR/RT/RC 四条序列记录
- 遵循系统惯例：soft delete via deleted_at、operator_id 审计、TIMESTAMPTZ 时间戳
- sales_order_items 包含 shipped_qty/returned_qty 跟踪字段

**Patterns to follow:** `abt/migrations/046_create_quotations.sql`

**Verification:** `cargo build` 通过

---

### U2. Proto Definitions

**Goal:** 定义销售订单、发货申请、销售退货、月对账单的 messages、enums、service RPCs

**Requirements:** R1-R9

**Dependencies:** None（可与 U1 并行）

**Files:**
- Create: `proto/abt/v1/sales_order.proto`
- Create: `proto/abt/v1/shipping.proto`
- Create: `proto/abt/v1/sales_return.proto`
- Create: `proto/abt/v1/reconciliation.proto`

**Approach:**
- 4 个独立 proto 文件，各自定义 enum + messages + service
- 引用 base.proto 的 PaginationParams/PaginationInfo/U64Response/BoolResponse
- 每个 service 包含 CRUD + List + UpdateStatus（对账单额外有 AddAdjustment）
- CreateRequest 不含系统生成字段（编号、状态、跟踪数量）
- UpdateSalesOrderRequest 仅含主信息字段（不含行项目）

**Patterns to follow:** `proto/abt/v1/quotation.proto`

**Verification:** `cargo build` 成功生成 proto 代码

---

### U3. Sales Order: Model + Repository + Service + Impl

**Goal:** 实现销售订单完整业务层

**Requirements:** R1, R2, R9

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/models/sales_order.rs`
- Create: `abt/src/repositories/sales_order_repo.rs`
- Create: `abt/src/service/sales_order_service.rs`
- Create: `abt/src/implt/sales_order_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model: SalesOrder + SalesOrderItem + SalesOrderQuery，手动 FromRow
- Repo: insert/update/update_header/soft_delete/find_by_id/query/query_count/update_status/insert_items/find_by_order_id/update_shipped_qty/update_returned_qty。query 支持 keyword（ILIKE order_no 或 customer_name）+ status 筛选 + 分页
- Service trait: create/update_header/delete/get_by_id/list/update_status
- Impl:
  - create: 若有 quotation_id 则校验报价单 Accepted 并复制行项目；生成 SO 编号；校验 product_id 存在性；计算 subtotal/total_amount
  - update_header: 任何状态都可修改主信息（不涉及行项目）
  - delete: 仅 Draft
  - update_status: Draft→Confirmed, Confirmed→Cancelled, Confirmed→InProgress, InProgress→Completed（状态白名单 matches!）
- 工厂函数: `get_sales_order_service`
- 新表使用运行时检查 sqlx（sqlx::query_as，非 sqlx::query! 宏）

**Patterns to follow:** `abt/src/implt/quotation_service_impl.rs`、`abt/src/repositories/quotation_repo.rs`

**Test scenarios:**
- Happy path: create 独立订单 → 生成编号 → 返回 ID
- Happy path: create 从 Accepted 报价单转入 → 复制行项目
- Happy path: update_header 修改客户信息（Confirmed 状态）
- Happy path: update_status Draft→Confirmed
- Error path: create 引用不存在的 quotation_id → NotFound
- Error path: create 引用非 Accepted 报价单 → BusinessValidation
- Error path: delete Confirmed 订单 → BusinessValidation
- Error path: update_status 非法转换（如 Completed→Draft）→ BusinessValidation

**Verification:** `cargo clippy` 通过

---

### U4. Shipping Request: Model + Repository + Service + Impl

**Goal:** 实现发货申请完整业务层，集成库存出库

**Requirements:** R3, R4, R9

**Dependencies:** U3

**Files:**
- Create: `abt/src/models/shipping_request.rs`
- Create: `abt/src/repositories/shipping_request_repo.rs`
- Create: `abt/src/service/shipping_request_service.rs`
- Create: `abt/src/implt/shipping_request_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model: ShippingRequest + ShippingRequestItem + ShippingRequestQuery，手动 FromRow
- Repo: insert/update/soft_delete/find_by_id/query/query_count/update_status/update_shipped_at/update_confirmed_at/insert_items/delete_by_request/find_by_request_id。query 支持 keyword + status + order_id 筛选
- Service trait: create/update/delete/get_by_id/list/update_status
- Impl:
  - create: 校验订单 Confirmed/InProgress；每行校验 quantity <= order_item.quantity - order_item.shipped_qty；从 order_item 冗余产品信息；生成 SR 编号
  - update: 仅 Pending 可改行项目，重新校验数量约束
  - update_status:
    - Pending→Confirmed: 记录 confirmed_at
    - Confirmed→Shipped: 记录 shipped_at，对每行调用 `InventoryService::stock_out(StockChangeRequest{ ref_order_type: "shipping_request", ref_order_id, ... })`，累加 order_item.shipped_qty
    - Pending→Cancelled
  - delete: 仅 Pending
- ShippingRequestServiceImpl 构造时注入 `Arc<PgPool>`，通过 `get_inventory_service(ctx)` 获取库存服务

**Patterns to follow:** `abt/src/implt/quotation_service_impl.rs`

**Deferred to implementation:** stock_out 需要的 location_id 来源（可能需要额外参数或默认库位逻辑）

**Test scenarios:**
- Happy path: create 发货申请 → 校验订单状态 → 校验数量 → 返回 ID
- Happy path: update_status Confirmed→Shipped → 调用库存出库 → 累加 shipped_qty
- Error path: create 订单为 Draft → BusinessValidation
- Error path: create 发货数量超过剩余可发量 → BusinessValidation
- Error path: update Confirmed 状态的发货申请 → BusinessValidation
- Error path: delete Shipped 发货申请 → BusinessValidation

**Verification:** `cargo clippy` 通过

---

### U5. Sales Return: Model + Repository + Service + Impl

**Goal:** 实现销售退货完整业务层，集成库存入库

**Requirements:** R5, R6, R9

**Dependencies:** U3, U4

**Files:**
- Create: `abt/src/models/sales_return.rs`
- Create: `abt/src/repositories/sales_return_repo.rs`
- Create: `abt/src/service/sales_return_service.rs`
- Create: `abt/src/implt/sales_return_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model: SalesReturn + SalesReturnItem + SalesReturnQuery，手动 FromRow
- Repo: insert/update/soft_delete/find_by_id/query/query_count/update_status/insert_items/delete_by_return/find_by_return_id。query 支持 keyword + status + order_id + request_id 筛选
- Service trait: create/update/delete/get_by_id/list/update_status
- Impl:
  - create: 校验发货单 Shipped；每行校验 quantity <= shipped_qty - 已退货数量（查询该 order_item 的所有已完成退货汇总）；从 shipping_request_item 获取产品信息和 unit_price；计算 subtotal/total_amount；生成 RT 编号
  - update: 仅 Pending 可改行项目
  - update_status:
    - Pending→Approved→Received→Completed
    - Pending/Approved→Rejected
    - Completed 时：对每行调用 `InventoryService::stock_in(StockChangeRequest{ ref_order_type: "sales_return", ... })`，累加 order_item.returned_qty
  - delete: 仅 Pending

**Patterns to follow:** `abt/src/implt/quotation_service_impl.rs`

**Deferred to implementation:** stock_in 需要的 location_id 来源；已退货数量汇总查询（可能需要单独 repo 方法）

**Test scenarios:**
- Happy path: create 退货 → 校验发货单 Shipped → 校验数量 → 返回 ID
- Happy path: update_status Received→Completed → 调用库存入库 → 累加 returned_qty
- Error path: create 发货单非 Shipped → BusinessValidation
- Error path: create 退货数量超过可退量 → BusinessValidation
- Error path: update Completed 退货单 → BusinessValidation
- Error path: update_status 非法转换（如 Rejected→Approved）→ BusinessValidation

**Verification:** `cargo clippy` 通过

---

### U6. Reconciliation: Model + Repository + Service + Impl

**Goal:** 实现月对账单完整业务层，自动汇总发货和退货明细

**Requirements:** R7, R8, R9

**Dependencies:** U4, U5

**Files:**
- Create: `abt/src/models/reconciliation.rs`
- Create: `abt/src/repositories/reconciliation_repo.rs`
- Create: `abt/src/service/reconciliation_service.rs`
- Create: `abt/src/implt/reconciliation_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model: ReconciliationStatement + ReconciliationItem + ReconciliationQuery，手动 FromRow
- Repo: insert/soft_delete/find_by_id/query/query_count/update_status/insert_items/find_by_statement_id/delete_adjustments_by_statement/recalculate_totals。recalculate_totals 用 SQL 聚合计算 shipping_total/return_total/adjustment_total/net_amount
- Service trait: create/add_adjustments/update/delete/get_by_id/list/update_status
- Impl:
  - create: 指定 customer_name + period_year + period_month，查询该月 Shipped 发货单明细（JOIN shipping_requests + shipping_request_items）和 Completed 退货单明细（JOIN sales_returns + sales_return_items），插入 reconciliation_items。计算汇总金额。生成 RC 编号。唯一约束防重复
  - add_adjustments: 仅 Draft 状态。先删除旧调整项（source_type='adjustment'），插入新调整项，重算汇总
  - update: 仅 Draft 可改 remark
  - update_status: Draft→Confirmed→Approved
  - delete: 仅 Draft

**Patterns to follow:** `abt/src/implt/quotation_service_impl.rs`

**Deferred to implementation:** 对账单自动汇总的 SQL 查询细节（需要 JOIN shipping_requests + shipping_request_items + sales_returns + sales_return_items + products）

**Test scenarios:**
- Happy path: create 对账单 → 自动汇总发货+退货明细 → 返回 ID
- Happy path: add_adjustments 添加正/负调整项 → 重算 net_amount
- Happy path: update_status Draft→Confirmed→Approved
- Error path: create 同客户同月重复 → Conflict
- Error path: add_adjustments Confirmed 状态 → BusinessValidation
- Error path: delete Confirmed 对账单 → BusinessValidation

**Verification:** `cargo clippy` 通过

---

### U7. gRPC Handlers + Server Registration

**Goal:** 实现 4 个 Proto 层到 Service 层的转换，注册到 gRPC server

**Requirements:** R1-R9

**Dependencies:** U2, U3, U4, U5, U6

**Files:**
- Create: `abt-grpc/src/handlers/sales_order.rs`
- Create: `abt-grpc/src/handlers/shipping_request.rs`
- Create: `abt-grpc/src/handlers/sales_return.rs`
- Create: `abt-grpc/src/handlers/reconciliation.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/server.rs`

**Approach:**
- 4 个 handler 文件，各自实现对应的 generated GrpcService trait
- 每个 handler：提取 request → AppState::get() → get_*_service → 调用 service → map_err(err_to_status) → 构造 response
- 写操作由 handler 管理 tx.commit()，service 通过 executor 操作
- Model→Proto 转换函数：每个模块定义 sales_order_to_proto, shipping_request_to_proto 等
- status_i16_to_proto / status_proto_to_i16 每个模块各一套
- server.rs 注册 4 个 ServiceServer with auth_interceptor
- Reconciliation handler 的 CreateReconciliation: 从 request 构造 ReconciliationStatement，service 层自动汇总

**Patterns to follow:** `abt-grpc/src/handlers/quotation.rs`

**Test scenarios:**
- Happy path: CreateSalesOrder 返回 ID
- Happy path: GetSalesOrder 返回含 items 的完整数据
- Happy path: ListShippingRequests 分页响应
- Happy path: CreateReconciliation 自动汇总明细
- Error path: CreateShippingRequest 订单为 Draft → gRPC FailedPrecondition
- Error path: UpdateSalesOrderStatus 非法转换 → gRPC FailedPrecondition

**Verification:** `cargo clippy` 通过，`cargo build` 通过

---

## System-Wide Impact

- **Interaction graph:** SalesOrderService 引用 QuotationRepo（校验报价单）和 ProductRepo（校验 product_id）。ShippingRequestServiceImpl 和 SalesReturnServiceImpl 引用 InventoryService（出库/入库）。ReconciliationServiceImpl 引用 ShippingRequestRepo 和 SalesReturnRepo（汇总明细）
- **Error propagation:** ServiceError::BusinessValidation / NotFound / Conflict 通过 err_to_status 转为 gRPC Status
- **State lifecycle risks:** 发货出库在同一事务内完成 shipped_qty 累加 + InventoryService::stock_out，避免部分更新。退货同理
- **API surface parity:** 新增 4 个独立 service，不影响现有 API
- **Unchanged invariants:** 现有 products/inventory/bom/quotation 模块不受影响

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 发货出库事务一致性 | shipped_qty 累加和 InventoryService::stock_out 在同一事务内 |
| 退货数量超发 | 查询 order_item 维度已退货总量做校验，非单次校验 |
| 对账单重复创建 | unique index (customer_name, period_year, period_month) |
| 库存集成需 location_id | 延迟到实现阶段确定来源（可能需额外参数或默认库位）|
| 编号并发冲突 | 复用已验证的 SELECT FOR UPDATE 机制 |

---

## Sources & References

- **Origin document:** docs/superpowers/specs/2026-05-20-sales-order-shipping-return-reconciliation-design.md
- **Preceding plan:** docs/plans/2026-05-20-002-feat-sales-quotation-plan.md
