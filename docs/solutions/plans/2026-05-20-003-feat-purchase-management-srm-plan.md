---
title: "feat: Purchase Management System (SRM)"
type: feat
status: active
date: 2026-05-20
origin: docs/superpowers/specs/2026-05-20-purchase-management-srm-design.md
---

# Purchase Management System (SRM)

## Summary

实现采购管理系统完整模块，包含供应商主档案、供应商价格登记簿、采购订单（含零星采购）、月度对账单、发票登记和付款申请。复用现有分层架构（Proto → Model → Repo → Service → Handler），引入文档编号服务为采购单/对账单/付款申请生成统一编号。

---

## Problem Frame

ABT 系统目前缺少采购管理能力。采购报价、下单、对账、付款全部在线下处理，无法追溯每笔采购的完整生命周期，也无法实现"入库单-发票-付款"三单匹配的财务闭环。

---

## Requirements

- R1. 供应商主档案 CRUD（主表 + 联系人 + 银行账户，分级分类管理）
- R2. 供应商价格登记簿（覆盖式报价 + 有效期控制）
- R3. 采购订单 CRUD（生产采购/零星采购，下单时快照价格，状态流转）
- R4. 月对账单自动生成（按供应商按月汇总已收货未对账的采购明细）
- R5. 发票登记与核验
- R6. 付款申请与状态流转
- R7. 文档编号服务（PO/PS/PP 三种类型，月度重置）
- R8. 权限集成（供应商/采购/采购结算三个资源）

---

## Scope Boundaries

- 物理收货入库由仓库模块处理，采购模块仅跟踪 `received_qty`
- 不包含 MRP 驱动的采购需求自动生成
- 不包含供应商协同门户和绩效自动评分
- 不包含采购框架协议管理
- 不包含审批流（状态由操作员手动推进）
- 不修改前端代码

### Deferred to Follow-Up Work

- 采购收货与仓库库存系统联动（回写 `received_qty`）：需要仓库模块配合，后续单独实现
- 供应商价格导入/导出 Excel：在基础 CRUD 稳定后加入

---

## Context & Research

### Relevant Code and Patterns

- **仓储层模式**：`abt/src/repositories/warehouse_repo.rs` — 无状态结构体，读操作用 `&PgPool`，写操作用 `Executor<'_>`，软删除 `SET deleted_at = NOW()`
- **Service trait 模式**：`abt/src/service/warehouse_service.rs` — `#[async_trait]` + `Send + Sync`
- **Service impl 模式**：`abt/src/implt/warehouse_service_impl.rs` — 持有 `Arc<PgPool>`，唯一约束冲突映射 sqlx error code `23505`
- **Handler 模式**：`abt-grpc/src/handlers/price.rs` — `AppState::get().await`，handler 管理事务，`err_to_status` 映射错误，Decimal 用字符串传输
- **分页模式**：`abt/src/repositories/mod.rs` — `PaginatedResult<T>`，`PaginationParams`
- **权限宏**：`abt-macros` — `#[require_permission(Resource::X, Action::Y)]`
- **模块注册**：`abt/src/lib.rs` 工厂函数 + `abt-grpc/src/server.rs` AppState 方法 + tonic 注册
- **模糊搜索**：`abt/src/repositories/mod.rs` — `build_fuzzy_pattern()`

### Institutional Learnings

- **事务安全**：读后写场景必须用 `SELECT ... FOR UPDATE`（采购订单快照价格时适用）— `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`
- **业务错误处理**：Handler 层用 `error::business_error()` 处理状态转换验证等预期业务错误，不 log — `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`
- **迁移安全**：用 `INSERT ... ON CONFLICT DO NOTHING`，归档表用 `RENAME TO _archived` — `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`
- **权限注册**：Proto `Resource` enum + `resources.rs` 的 `RESOURCES` 数组 + 一致性测试 — `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md`
- **权限宏用法**：每个 handler 方法加 `#[require_permission(Resource::Xxx, Action::Yyy)]` — `docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md`

---

## Key Technical Decisions

- **文档编号服务作为第一个实现单元**：采购订单、对账单、付款申请都依赖编号生成。虽然销售报价模块也设计了此服务，但尚未实现，SRM 需要先建好这个基础设施
- **三个 Proto 文件按业务域分**：`supplier.proto`（供应商）、`purchase.proto`（报价+订单）、`purchase_settlement.proto`（对账+发票+付款），避免单文件过大
- **供应商价格采用追加行+有效期模式**：每次报价新增一行，通过 `valid_until` 控制有效性。下单时将当前有效价格快照到采购订单行项目，结算按快照价格
- **采购收货与仓库解耦**：采购模块只维护 `received_qty` 字段，物理入库由仓库模块处理（`ref_order_type = "purchase_order"`）。后续迭代再实现联动回写
- **对账单明细关联采购订单**：通过 `purchase_statement_items.po_id` 关联，生成时查找状态为 FullyReceived/PartialReceived 且未被对账关联的订单
- **Decimal 在 Proto 中用字符串传输**：与现有价格模块保持一致

---

## Open Questions

### Resolved During Planning

- **文档编号依赖**：`document_sequences` 表尚未实现，SRM 将包含其实现
- **迁移编号**：045/046 已被报价单模块占用，SRM 从 047 开始
- **权限资源划分**：三个资源（SUPPLIER, PURCHASE, PURCHASE_SETTLEMENT）对应三个 proto service

### Deferred to Implementation

- **对账单生成时如何精确判断"未被关联"**：实现时需确定是用采购订单状态（Reconciled）还是维护额外关联表
- **received_qty 回写机制**：需要与仓库模块协调，本期先手动维护

---

## Output Structure

```
proto/abt/v1/
  supplier.proto
  purchase.proto
  purchase_settlement.proto

abt/src/models/
  document_sequence.rs
  supplier.rs
  supplier_price.rs
  purchase_order.rs
  purchase_settlement.rs

abt/src/repositories/
  document_sequence_repo.rs
  supplier_repo.rs
  supplier_price_repo.rs
  purchase_order_repo.rs
  purchase_settlement_repo.rs

abt/src/service/
  document_sequence_service.rs
  supplier_service.rs
  supplier_price_service.rs
  purchase_order_service.rs
  statement_service.rs
  invoice_service.rs
  payment_service.rs

abt/src/implt/
  document_sequence_service_impl.rs
  supplier_service_impl.rs
  supplier_price_service_impl.rs
  purchase_order_service_impl.rs
  statement_service_impl.rs
  invoice_service_impl.rs
  payment_service_impl.rs

abt-grpc/src/handlers/
  supplier.rs
  purchase.rs
  purchase_settlement.rs

abt/migrations/
  047_create_document_sequences.sql
  048_create_supplier_tables.sql
  049_create_purchase_tables.sql
```

---

## Implementation Units

### U1. Proto Definitions

**Goal:** 定义三个 proto 文件，编译生成 Rust 代码

**Requirements:** R1, R2, R3, R4, R5, R6

**Dependencies:** None

**Files:**
- Create: `proto/abt/v1/supplier.proto`
- Create: `proto/abt/v1/purchase.proto`
- Create: `proto/abt/v1/purchase_settlement.proto`

**Approach:**
- 完全按照设计规格中的 proto 定义编写
- `supplier.proto`：`SupplierService` + 供应商/联系人/银行账户的 CRUD messages
- `purchase.proto`：`PurchaseService` + 供应商报价 + 采购订单 RPC
- `purchase_settlement.proto`：`PurchaseSettlementService` + 对账单/发票/付款 RPC
- 所有 Decimal 字段用 `string` 类型（与 price.proto 一致）
- 时间戳用 `int64`（UNIX epoch）
- 日期（period_start/period_end、invoice_date）用 `int64`
- 运行 `cargo build` 触发 `build.rs` 重新生成 proto 代码

**Patterns to follow:**
- `proto/abt/v1/price.proto` — Decimal 字段用 string
- `proto/abt/v1/base.proto` — `PaginationParams`、`PaginationInfo`、`DeleteRequest`
- 销售报价设计中 quotation.proto 的结构（状态 enum、CRUD messages）

**Test scenarios:**
- Test expectation: none — proto 编译通过即验证

**Verification:**
- `cargo build` 成功，`abt-grpc/src/generated/` 中出现三个新 proto 的生成代码

---

### U2. Database Migrations

**Goal:** 创建所有 SRM 相关数据库表和文档编号种子数据

**Requirements:** R7, R1, R2, R3, R4, R5, R6

**Dependencies:** U1

**Files:**
- Create: `abt/migrations/047_create_document_sequences.sql`
- Create: `abt/migrations/048_create_supplier_tables.sql`
- Create: `abt/migrations/049_create_purchase_tables.sql`

**Approach:**
- 047: `document_sequences` 表 + PO/PS/PP 三行种子数据（`INSERT ... ON CONFLICT DO NOTHING`）
- 048: `suppliers` + `supplier_contacts` + `supplier_bank_accounts` + `supplier_prices` 四张表 + 索引
- 049: `purchase_orders` + `purchase_order_items` + `purchase_statements` + `purchase_statement_items` + `purchase_invoices` + `purchase_payments` 六张表 + 索引
- 所有表遵循项目约定：`BIGSERIAL PRIMARY KEY`、`TIMESTAMPTZ` 时间戳、软删除 `deleted_at`、Decimal 精度 `(14,6)` 单价/数量、`(14,2)` 金额
- 不使用外键约束（项目约定：应用层保证引用完整性）

**Patterns to follow:**
- `abt/migrations/036_products_table_redesign.sql` — 表结构风格
- `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md` — 迁移安全实践

**Test scenarios:**
- Test expectation: none — 纯 DDL 迁移文件，通过 `cargo build` + `cargo clippy` 间接验证

**Verification:**
- 迁移文件存在且 `sqlx` 可以连接数据库执行

---

### U3. Document Sequence Module

**Goal:** 实现文档编号服务，为采购单/对账单/付款申请生成唯一编号

**Requirements:** R7

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/models/document_sequence.rs`
- Create: `abt/src/repositories/document_sequence_repo.rs`
- Create: `abt/src/service/document_sequence_service.rs`
- Create: `abt/src/implt/document_sequence_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- `DocumentSequenceRepo::next_number(executor, doc_type)` — `SELECT ... FOR UPDATE` 锁行 → `current_value + 1` → 格式化 `{prefix}{year}-{month}-{序号}` → `UPDATE`
- `DocumentSequenceRepo::ensure_sequence(executor, doc_type, prefix, reset_rule)` — 幂等初始化
- `DocumentSequenceService` trait 只有 `next_number` 方法
- `DocumentSequenceServiceImpl` 持有 `Arc<PgPool>`（虽然 next_number 接收 executor，但 trait 一致性需要）
- 编号格式 `PO202605-00001`，月度重置时检查 `updated_at` 是否跨月
- 在 `lib.rs` 添加 `get_document_sequence_service` 工厂函数

**Patterns to follow:**
- `abt/src/implt/warehouse_service_impl.rs` — impl 持有 `Arc<PgPool>` 模式
- `abt/src/lib.rs` — 工厂函数注册模式

**Test scenarios:**
- Happy path: `next_number("PO")` 返回 `PO202605-00001`，连续调用返回 `00002`、`00003`
- Edge case: 跨月重置序号回到 1（月份数字变化时 current_value 归零）
- Edge case: 不存在的 doc_type 应返回错误或自动创建
- Integration: 在事务中调用，事务回滚时编号不被消耗

**Verification:**
- `cargo clippy` 通过，模块注册完整

---

### U4. Supplier Module

**Goal:** 实现供应商主档案完整 CRUD（主表 + 联系人 + 银行账户）

**Requirements:** R1

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/models/supplier.rs`
- Create: `abt/src/repositories/supplier_repo.rs`
- Create: `abt/src/service/supplier_service.rs`
- Create: `abt/src/implt/supplier_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model：`Supplier`（FromRow）、`SupplierContact`（FromRow）、`SupplierBankAccount`（FromRow）、`SupplierQuery`
- Repo：
  - `SupplierRepo` — insert/update/soft_delete/find_by_id/query/query_count/update_status
  - `SupplierContactRepo` — insert_batch/delete_by_supplier/find_by_supplier
  - `SupplierBankAccountRepo` — insert_batch/delete_by_supplier/find_by_supplier
- Service：
  - `create` — 事务内插入主表 + 批量插入联系人/银行账户
  - `update` — 事务内更新主表 + 删除旧子表 + 批量插入新子表（整体替换）
  - `delete` — 校验无采购订单引用后软删除
  - `get_by_id` — 查主表 + 二次查询填充 contacts/bank_accounts
  - `list` — 模糊搜索 supplier_name/supplier_code，分页
  - `update_status` — 状态切换
- 唯一约束冲突（`supplier_code`）映射为 `ServiceError::Conflict`

**Patterns to follow:**
- `abt/src/implt/warehouse_service_impl.rs` — service impl 整体结构
- `abt/src/repositories/warehouse_repo.rs` — 软删除、查询模式
- `abt/src/implt/product_service_impl.rs` — `map_duplicate_error` 模式

**Test scenarios:**
- Happy path: 创建供应商带联系人和银行账户，查询返回完整数据
- Happy path: 更新供应商替换所有联系人/银行账户
- Happy path: 列表分页 + 关键词搜索
- Edge case: `supplier_code` 重复创建返回 Conflict 错误
- Error path: 删除被采购订单引用的供应商返回业务错误
- Edge case: 多个联系人中只有一个 `is_primary = true`

**Verification:**
- `cargo clippy` 通过，`get_supplier_service` 工厂函数注册

---

### U5. Supplier Price Module

**Goal:** 实现供应商价格登记簿（追加行 + 有效期）

**Requirements:** R2

**Dependencies:** U1, U2, U4

**Files:**
- Create: `abt/src/models/supplier_price.rs`
- Create: `abt/src/repositories/supplier_price_repo.rs`
- Create: `abt/src/service/supplier_price_service.rs`
- Create: `abt/src/implt/supplier_price_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model：`SupplierPrice`（FromRow）、`SupplierPriceQuery`
- Repo：
  - `insert` — 追加新报价行
  - `find_active` — 查询指定供应商+物料的当前有效报价（`NOW() BETWEEN valid_from AND valid_until`）
  - `query/query_count` — 支持按供应商/物料筛选，`active_only` 过滤
- Service：
  - `upsert` — 插入新报价行（不删除旧报价），返回 `price_id`
  - `list` — 分页查询，冗余填充 product_code/product_name/unit
- 查询列表时 JOIN products 表获取 product_code/product_name/unit

**Patterns to follow:**
- `abt/src/repositories/product_price_repo.rs` — 价格追加日志模式、分页
- `abt/src/implt/product_service_impl.rs` — 唯一约束冲突映射

**Test scenarios:**
- Happy path: 登记报价，查询返回含产品冗余信息
- Happy path: `active_only = true` 只返回当前有效报价
- Edge case: 同一供应商同一物料多次报价，全部保留
- Edge case: `find_active` 返回最新一条有效报价

**Verification:**
- `cargo clippy` 通过，`get_supplier_price_service` 工厂函数注册

---

### U6. Purchase Order Module

**Goal:** 实现采购订单完整 CRUD（含零星采购）和状态流转

**Requirements:** R3

**Dependencies:** U1, U2, U3, U4, U5

**Files:**
- Create: `abt/src/models/purchase_order.rs`
- Create: `abt/src/repositories/purchase_order_repo.rs`
- Create: `abt/src/service/purchase_order_service.rs`
- Create: `abt/src/implt/purchase_order_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model：`PurchaseOrder`（FromRow）、`PurchaseOrderItem`（FromRow）、`PurchaseOrderQuery`
- Repo：
  - `insert/update/soft_delete/find_by_id/query/query_count/update_status`
  - `insert_items/delete_by_po/find_items_by_po` — 行项目管理
- Service：
  - `create` — 事务内：生成编号 → 校验 product_id 存在性 → 冗余写入 product_code/name/unit → 计算 subtotal/total_amount → 插入主表+行项目
  - `update` — 仅 Draft 状态可编辑，整体替换行项目
  - `delete` — 仅 Draft 状态可删除
  - `update_status` — 状态转换白名单：Draft→Submitted→Approved→PartialReceived/FullyReceived→Reconciled→Closed
  - `get_by_id/list` — 查主表后二次查询填充 items，list JOIN suppliers 获取 supplier_name
- 状态转换失败用 `ServiceError::BusinessValidation`

**Patterns to follow:**
- 销售报价设计中的 `QuotationServiceImpl` — 编号生成 + 行项目 + 状态流转模式
- `abt/src/implt/warehouse_service_impl.rs` — 基础 CRUD 模式

**Test scenarios:**
- Happy path: 创建采购订单，编号自动生成，行项目快照价格和产品信息
- Happy path: 状态流转 Draft→Submitted→Approved
- Edge case: 非 Draft 状态 update/delete 返回 BusinessValidation 错误
- Edge case: 无效状态转换（如 Draft→FullyReceived）被拒绝
- Integration: 编号生成在事务中，回滚时编号不被消耗

**Verification:**
- `cargo clippy` 通过，`get_purchase_order_service` 工厂函数注册

---

### U7. Purchase Settlement Module

**Goal:** 实现月对账单生成、发票登记和付款申请

**Requirements:** R4, R5, R6

**Dependencies:** U1, U2, U3, U4, U6

**Files:**
- Create: `abt/src/models/purchase_settlement.rs`
- Create: `abt/src/repositories/purchase_settlement_repo.rs`
- Create: `abt/src/service/statement_service.rs`
- Create: `abt/src/service/invoice_service.rs`
- Create: `abt/src/service/payment_service.rs`
- Create: `abt/src/implt/statement_service_impl.rs`
- Create: `abt/src/implt/invoice_service_impl.rs`
- Create: `abt/src/implt/payment_service_impl.rs`
- Modify: `abt/src/models/mod.rs`
- Modify: `abt/src/repositories/mod.rs`
- Modify: `abt/src/service/mod.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- Model：`PurchaseStatement`、`StatementItem`、`PurchaseInvoice`、`PurchasePayment` + 各自 Query 结构体
- Repo：`StatementRepo`、`InvoiceRepo`、`PaymentRepo` — 各自的 CRUD + 状态更新
- StatementService：
  - `generate` — 事务内：生成编号 → 查找该供应商在指定期间内 FullyReceived/PartialReceived 状态的采购订单 → 汇总生成明细 → 更新关联采购订单状态为 Reconciled → 计算 total_amount
  - `get_by_id` — 查主表 + 二次查询填充 items，JOIN suppliers 获取 supplier_name
- InvoiceService：
  - `create` — 登记发票，关联对账单（可选）
  - `list` — 分页查询，JOIN suppliers + statements 获取冗余名称
  - `update_status` — Registered→Verified
- PaymentService：
  - `create` — 生成编号，创建付款申请
  - `list` — 分页查询，JOIN suppliers + invoices 获取冗余信息
  - `update_status` — Pending→Approved→Paid

**Patterns to follow:**
- `abt/src/implt/warehouse_service_impl.rs` — 基础 CRUD + 状态更新模式
- 对账单生成参考采购订单的编号生成 + 事务管理模式

**Test scenarios:**
- Happy path: 生成对账单，自动汇总采购订单明细
- Happy path: 发票登记 → 付款申请 → 状态流转完整闭环
- Edge case: 对账单期间无已收货订单时返回空对账单或错误
- Edge case: 同一采购订单不可被两次对账
- Edge case: 发票金额与对账单金额不匹配时是否校验（本期不强制）
- Integration: 对账单生成后，关联采购订单状态变为 Reconciled

**Verification:**
- `cargo clippy` 通过，`get_statement_service`、`get_invoice_service`、`get_payment_service` 工厂函数注册

---

### U8. gRPC Handlers + Registration

**Goal:** 实现三个 handler 文件，完成服务注册和模块导出

**Requirements:** R1, R2, R3, R4, R5, R6

**Dependencies:** U3, U4, U5, U6, U7

**Files:**
- Create: `abt-grpc/src/handlers/supplier.rs`
- Create: `abt-grpc/src/handlers/purchase.rs`
- Create: `abt-grpc/src/handlers/purchase_settlement.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/server.rs`

**Approach:**
- 每个 handler 实现对应 proto 生成的 service trait
- 每个方法：`#[require_permission(Resource::X, Action::Y)]` → `AppState::get().await` → 创建 service → 调用 → 转换响应
- 写操作在 handler 层管理事务：`state.begin_transaction()` → 传 executor → `tx.commit()`
- Proto ↔ Model 转换：
  - Decimal ↔ string（`.to_string()` / `.parse::<Decimal>()`）
  - 时间戳 ↔ i64（`.timestamp()` / `NaiveDateTime::from_timestamp_opt()`）
  - 日期 ↔ i64（epoch days）
  - 状态 enum ↔ i16（match 映射）
- `server.rs` 注册三个 tonic service

**Patterns to follow:**
- `abt-grpc/src/handlers/price.rs` — handler 结构体、事务管理、Decimal 转换、错误映射
- `abt-grpc/src/handlers/product.rs` — 列表查询 handler
- `abt-grpc/src/server.rs` — service 注册模式

**Test scenarios:**
- Test expectation: none — handler 层通过 `cargo clippy` 和实际 gRPC 调用验证

**Verification:**
- `cargo clippy` 通过
- `cargo build` 成功
- 服务启动后 gRPC reflection 能发现三个新 service

---

### U9. Permission Registration

**Goal:** 在权限系统中注册 SRM 的三个资源

**Requirements:** R8

**Dependencies:** U1

**Files:**
- Modify: `proto/abt/v1/permission.proto` — Resource enum 添加 SUPPLIER/PURCHASE/PURCHASE_SETTLEMENT
- Modify: `abt/src/models/resources.rs` — RESOURCES 数组添加三个资源的所有 action

**Approach:**
- 在 `Resource` enum 添加 `SUPPLIER`、`PURCHASE`、`PURCHASE_SETTLEMENT`（接续现有最大值）
- 在 `resources.rs` 的 `RESOURCES` 数组添加每个资源的 READ/WRITE/DELETE 条目及中文名称
- 运行 `cargo test` 确保一致性测试 `all_resource_codes_match_resources_rs()` 通过

**Patterns to follow:**
- `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md`
- 现有 `Resource` enum 和 `RESOURCES` 数组结构

**Test scenarios:**
- Happy path: 一致性测试 `all_resource_codes_match_resources_rs()` 通过
- Edge case: 新增 enum 值不与现有值冲突

**Verification:**
- `cargo test -p abt` 一致性测试通过
- `cargo clippy` 通过

---

## System-Wide Impact

- **Interaction graph:** 采购订单与库存系统通过 `ref_order_type = "purchase_order"` 关联（仓库模块写入，采购模块读取汇总）
- **Error propagation:** Handler 层 `err_to_status` / `sqlx_err_to_status` / `business_error` 三层错误映射
- **State lifecycle risks:** 采购订单状态机需严格校验转换白名单，防止跳跃转换
- **API surface parity:** 三个 gRPC service 对应三个业务域，各自独立
- **Integration coverage:** 对账单生成是最复杂的跨表操作（查询采购订单 → 汇总 → 更新状态），需事务保证
- **Unchanged invariants:** 现有库存、产品、仓库模块的行为不变，SRM 是纯增量模块

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| `document_sequences` 与销售报价模块冲突 | 使用 `INSERT ... ON CONFLICT DO NOTHING` 种子数据，两个模块可并行开发 |
| 对账单生成逻辑复杂（多表关联+状态更新） | 在单个事务中执行全部操作，使用 `SELECT ... FOR UPDATE` 锁定关联采购订单 |
| `received_qty` 暂无自动回写机制 | 文档中明确标注为 deferred，本期手动维护 |
| Proto 编译缓存导致新文件不生成 | `cargo clean && cargo build` 兜底 |

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-20-purchase-management-srm-design.md](docs/superpowers/specs/2026-05-20-purchase-management-srm-design.md)
- Related code: `abt/src/repositories/warehouse_repo.rs` (CRUD pattern)
- Related code: `abt-grpc/src/handlers/price.rs` (handler pattern)
- Related code: `abt/src/implt/product_service_impl.rs` (duplicate error mapping)
- Learning: `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`
- Learning: `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`
- Learning: `docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md`
