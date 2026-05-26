---
title: "feat: Implement Sales Module (CRM) in abt-core"
type: feat
status: active
date: 2026-05-24
origin: docs/superpowers/specs/2026-05-24-sales-module-design.md
---

# feat: Implement Sales Module (CRM) in abt-core

## Summary

严格按照 `01-sales.html` 设计文档，在 `abt-core` crate 中实现 5 个销售子模块（Quotation → SalesOrder → ShippingRequest → SalesReturn → Reconciliation），每个遵循 migration → model → repo → service trait → impl 的分层模式，完整集成共享服务层。

---

## Problem Frame

ABT 系统需要完整的销售 CRM 流程：从报价到订单、发货、退货、对账。设计文档 `docs/uml-design/01-sales.html` 已完成详细设计，`abt-core/src/sales/` 已有骨架文件但全部为空。需要按设计规范填充完整实现。

---

## Requirements

- R1. 实现 Quotation 报价单模块（5 个状态：Draft/Sent/Accept/Reject/Expired，CRUD + 状态流转 + 批量过期）
- R2. 实现 SalesOrder 销售订单模块（7 个状态，含 create_from_quotation 派生、confirm 时库存预留）
- R3. 实现 ShippingRequest 发货申请模块（5 个状态，含 QMS OQC gate、ship 时 CostEntry COGS）
- R4. 实现 SalesReturn 销售退货模块（6 个状态 + 3 种 ReturnDisposition，含质检触发）
- R5. 实现 Reconciliation 月对账模块（5 个状态，含自动聚合发货明细、FMS 集成）
- R6. 所有子模块集成共享服务：DocumentSequence、StateMachine、AuditLog、EventBus
- R7. 所有 Service 方法返回 `Result<T, DomainError>`
- R8. 所有列表查询返回 `PaginatedResult<T>`

---

## Scope Boundaries

- gRPC handler 层（`abt-grpc`）不在本计划内
- Proto 定义（`proto/`）不在本计划内
- 前端代码（`E:\work\front\abt_front`）禁止修改
- 其他模块（Purchase、WMS、MES 等）不在本计划内
- 状态机转换规则数据（只需调用 API，不配置转换规则）

---

## Context & Research

### Relevant Code and Patterns

- `abt-core/src/master_data/customer/implt/mod.rs` — 完整的 ServiceImpl DI 模式参考（含 doc_seq、state_machine、audit、event_bus 集成）
- `abt-core/src/master_data/customer/repo.rs` — Repo SQL 模式参考（动态 SET 子句、分页查询、软删除）
- `abt-core/src/master_data/customer/model.rs` — Model 模式参考（struct + enum + request/query types）
- `abt-core/src/shared/enums/document_type.rs` — 已包含销售模块的 5 个 DocumentType
- `abt-core/src/shared/enums/event.rs` — 已包含 3 个销售相关 DomainEventType
- `abt-core/src/shared/enums/link_type.rs` — 已包含 DerivedFrom/Triggers/References/Reconciles
- `abt-core/src/shared/types/error.rs` — DomainError 统一错误模型
- `abt-core/src/shared/types/context.rs` — ServiceContext（含 reborrow 模式）
- `abt-core/src/shared/types/pagination.rs` — PaginatedResult + DataScope

### Institutional Learnings

- 无 `docs/solutions/` 中与销售模块直接相关的学习记录

### External References

- 无（代码库有充足的本地模式参考）

---

## Key Technical Decisions

- **Model 枚举定义在各子模块 model.rs 中**（非 shared/enums/）：QuotationStatus、SalesOrderStatus 等是销售域内部枚举，不跨模块共享，遵循 CustomerStatus 在 customer/model.rs 中的先例
- **Repo 使用动态 SQL 拼接**：Update 方法通过 `Vec<String>` 构建动态 SET 子句，匹配 CustomerRepo 的模式
- **跨模块依赖通过 `Arc<dyn Trait>` 注入**：SalesOrderServiceImpl 持有 `Arc<dyn QuotationService>`，而非直接访问 repo，遵循模块边界规则
- **Migration 编号从 002 开始**：001 已用于 shared infrastructure
- **items 更新策略：先删后插**：Quotation/SalesOrder 的明细行更新采用 delete_all + create_batch，简化实现并避免 diff 计算

---

## Open Questions

### Resolved During Planning

- 实现顺序：串行按子模块（Quotation → SalesOrder → ShippingRequest → SalesReturn → Reconciliation）
- 共享服务集成：完整集成，不使用 TODO 占位
- 跨模块依赖：Master Data 已实现，直接引用 Service trait

### Deferred to Implementation

- 状态机转换规则配置：需要在 DB 中配置各实体类型的合法状态转换（`state_transition_defs` 表），此工作可在集成测试时完成
- DomainEventType 扩展：当前只有 3 个销售事件类型，新事件（如 QuotationCreated、ShipmentShipped）可能需要添加到 enum 中。各子模块需要的事件：U1(QuotationCreated/Submitted/Accepted/Rejected/Expired)、U2(SalesOrderConfirmed/Cancelled)、U3(ShipmentShipped)、U4(SalesReturnReceived)、U5(ReconciliationConfirmed/Settled)。可在每个单元实现时按需添加到 `shared/enums/event.rs`
- QMS OQC gate：U3 ShippingRequest.confirm 依赖 QMS 模块（尚未实现），实现时先跳过 gate 检查，作为 placeholder 留待 QMS 模块完成后集成
- CostEntityType 选择：U3 ship 时记录 COGS 使用 `CostEntityType::SalesOrder`（已有枚举值=3），以父订单 ID 作为 entity_id，无需新增枚举变体

---

## Output Structure

```
abt-core/
  migrations/
    002_create_sales_quotation.sql
    003_create_sales_order.sql
    004_create_shipping_request.sql
    005_create_sales_return.sql
    006_create_reconciliation.sql
  src/sales/
    mod.rs                              # 已存在，pub mod 声明
    quotation/
      mod.rs                            # 已存在，pub use 重新导出
      model.rs                          # 已存在（空），填充
      repo.rs                           # 已存在（空），填充
      service.rs                        # 已存在（骨架），填充
      implt/mod.rs                      # 已存在（空），填充
    sales_order/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs  # 同上
    shipping_request/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs  # 同上
    sales_return/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs  # 同上
    reconciliation/
      mod.rs / model.rs / repo.rs / service.rs / implt/mod.rs  # 同上
```

---

## Implementation Units

### U1. Quotation 报价单

**Goal:** 实现完整的报价单模块（migration + model + repo + service trait + impl），含 CRUD、状态流转（Draft→Sent→Accepted/Rejected/Expired）、批量过期、共享服务集成

**Requirements:** R1, R6, R7, R8

**Dependencies:** None

**Files:**
- Create: `abt-core/migrations/002_create_sales_quotation.sql`
- Modify: `abt-core/src/sales/quotation/model.rs`
- Modify: `abt-core/src/sales/quotation/repo.rs`
- Modify: `abt-core/src/sales/quotation/service.rs`
- Modify: `abt-core/src/sales/quotation/implt/mod.rs`
- Modify: `abt-core/src/sales/quotation/mod.rs`

**Approach:**
- Migration: 创建 `quotations` 主表 + `quotation_items` 明细表，status 用 SMALLINT，标准审计字段
- Model: Quotation/QuotationItem（sqlx::FromRow）、QuotationStatus（#[repr(i16)] 枚举）、Create/Update/Query 请求结构体
- Repo: QuotationRepo（CRUD + 分页查询 + 批量过期）+ QuotationItemRepo（batch create + find + delete）
- Service trait: 9 个方法（create/find_by_id/update/submit/accept/reject/expire/batch_expire_overdue/list）
- Service impl: 注入 doc_seq/state_machine/audit/event_bus/customer_svc，create 时生成编号+校验 contact+审计+事件

**Patterns to follow:**
- `abt-core/src/master_data/customer/implt/mod.rs` — 完整的 DI + 共享服务集成模式
- `abt-core/src/master_data/customer/repo.rs` — 动态 SQL + 分页模式
- `abt-core/src/master_data/customer/model.rs` — struct/enum/request pattern
- `abt-core/src/shared/enums/document_type.rs` — 枚举 sqlx/serde 实现

**Test scenarios:**
- Happy path: create quotation with items → verify doc_number generated, status=Draft, amounts calculated
- Happy path: submit Draft quotation → status=Sent
- Happy path: accept Sent quotation → status=Accepted
- Happy path: reject Sent quotation → status=Rejected
- Happy path: expire quotation → status=Expired
- Happy path: batch_expire_overdue expires Sent quotations past valid_until
- Happy path: list with filters returns PaginatedResult
- Edge case: create with empty items succeeds (items validated on submit, not create)
- Error path: update non-Draft quotation → BusinessRule error
- Error path: submit quotation with no items → BusinessRule error
- Error path: accept non-Sent quotation → state machine rejects
- Error path: create with valid_until <= today → Validation error

**Verification:**
- `cargo clippy -p abt-core` 通过
- 所有 service trait 方法签名与 `01-sales.html` 一致
- 共享服务集成点（doc_seq/audit/event_bus/state_machine）均有调用

---

### U2. SalesOrder 销售订单

**Goal:** 实现完整的销售订单模块，含 create_from_quotation 派生、confirm 时库存预留、DocumentLink 关联

**Requirements:** R2, R6, R7, R8

**Dependencies:** U1

**Files:**
- Create: `abt-core/migrations/003_create_sales_order.sql`
- Modify: `abt-core/src/sales/sales_order/model.rs`
- Modify: `abt-core/src/sales/sales_order/repo.rs`
- Modify: `abt-core/src/sales/sales_order/service.rs`
- Modify: `abt-core/src/sales/sales_order/implt/mod.rs`
- Modify: `abt-core/src/sales/sales_order/mod.rs`

**Approach:**
- Migration: `sales_orders` 主表（含 delivered_qty/returned_qty 跟踪列）+ `sales_order_items` 明细表
- Model: SalesOrder/SalesOrderItem、SalesOrderStatus（7 个状态）、CreateSalesOrderReq/CreateFromQuotationReq
- Repo: 标准 CRUD + update_delivered_qty/update_returned_qty 辅助方法
- Service trait: 8 个方法（create/create_from_quotation/find_by_id/update_header/confirm/start_progress/complete/cancel/list）
- Service impl: 额外注入 QuotationService（create_from_quotation）、DocumentLinkService（DERIVED_FROM）、InventoryReservationService（confirm 时 Soft 预留 TTL=7d）
- create_from_quotation: 校验 quotation Accepted + 未过期 → 复制 items/customer/contact/sales_rep → doc_link(DerivedFrom)

**Patterns to follow:**
- U1（Quotation）的分层模式
- `abt-core/src/shared/document_link/service.rs` — DocumentLink 集成
- `abt-core/src/shared/inventory_reservation/service.rs` — InventoryReservation 集成

**Test scenarios:**
- Happy path: create order manually → status=Draft
- Happy path: create_from_quotation with Accepted quotation → copies items, creates doc link
- Happy path: confirm Draft → status=Confirmed, triggers inventory reservation
- Happy path: start_progress Confirmed → InProduction
- Happy path: complete with all items delivered → Completed
- Happy path: cancel Draft/Confirmed → Cancelled, releases reservation
- Edge case: complete with undelivered items → BusinessRule error
- Error path: create_from_quotation with non-Accepted quotation → BusinessRule error
- Error path: create_from_quotation with expired quotation → BusinessRule error
- Error path: cancel InProduction order → state machine rejects
- Integration: confirm triggers InventoryReservationService.reserve()

**Verification:**
- `cargo clippy -p abt-core` 通过
- create_from_quotation 正确引用 QuotationService trait
- DocumentLink 集成（DERIVED_FROM）
- InventoryReservation 集成（confirm 时 reserve，cancel 时 release）

---

### U3. ShippingRequest 发货申请

**Goal:** 实现完整的发货申请模块，含从订单创建、QMS OQC gate、发货时 COGS 成本记录

**Requirements:** R3, R6, R7, R8

**Dependencies:** U2

**Files:**
- Create: `abt-core/migrations/004_create_shipping_request.sql`
- Modify: `abt-core/src/sales/shipping_request/model.rs`
- Modify: `abt-core/src/sales/shipping_request/repo.rs`
- Modify: `abt-core/src/sales/shipping_request/service.rs`
- Modify: `abt-core/src/sales/shipping_request/implt/mod.rs`
- Modify: `abt-core/src/sales/shipping_request/mod.rs`

**Approach:**
- Migration: `shipping_requests` 主表 + `shipping_request_items` 明细表（含 warehouse_id、shipped_qty）
- Model: ShippingRequest/ShippingRequestItem、ShippingStatus（5 个状态）、CreateFromOrderReq（含 items 列表指定发货数量）
- Repo: 标准 CRUD + 辅助方法
- Service trait: 7 个方法（create_from_order/find_by_id/update/confirm/pick/ship/cancel/list）
- Service impl: 注入 SalesOrderService、DocumentLinkService（TRIGGERS）、InventoryReservationService（confirm 时释放、ship 时 fulfill）、CostEntryService（COGS）
- ship 流程: 更新 order_item.shipped_qty → inv_res.fulfill() → cost_entry.create_entries(COGS) → event_bus.publish(ShipmentShipped)

**Patterns to follow:**
- U2 的跨模块集成模式
- `abt-core/src/shared/cost_entry/service.rs` — CostEntry 集成模式

**Test scenarios:**
- Happy path: create_from_order with Confirmed order → status=Draft, doc link created
- Happy path: confirm → status=Confirmed (QMS OQC gate 为 placeholder，QMS 未实现时跳过)
- Happy path: pick → status=Picking
- Happy path: ship → status=Shipped, updates order_item.shipped_qty, triggers COGS + event
- Happy path: cancel Draft/Confirmed → Cancelled
- Edge case: create_from_order with requested_qty > remaining order qty → BusinessRule error
- Error path: create_from_order with Draft order → BusinessRule error
- Error path: ship Picking → success, ship Confirmed → state machine rejects
- Integration: ship triggers CostEntryService + InventoryReservationService.fulfill()

**Verification:**
- `cargo clippy -p abt-core` 通过
- ship 正确更新 order_item.shipped_qty
- DocumentLink（TRIGGERS）+ CostEntry（COGS）+ EventBus 集成完整

---

### U4. SalesReturn 销售退货

**Goal:** 实现完整的销售退货模块，含退货审批、收货、质检触发、按 disposition 处理

**Requirements:** R4, R6, R7, R8

**Dependencies:** U3

**Files:**
- Create: `abt-core/migrations/005_create_sales_return.sql`
- Modify: `abt-core/src/sales/sales_return/model.rs`
- Modify: `abt-core/src/sales/sales_return/repo.rs`
- Modify: `abt-core/src/sales/sales_return/service.rs`
- Modify: `abt-core/src/sales/sales_return/implt/mod.rs`
- Modify: `abt-core/src/sales/sales_return/mod.rs`

**Approach:**
- Migration: `sales_returns` 主表 + `sales_return_items` 明细表（含 disposition SMALLINT 枚举）
- Model: SalesReturn/SalesReturnItem、ReturnStatus（6 个状态）、ReturnDisposition（Restock/Scrap/Rework）、CreateSalesReturnReq
- Repo: 标准 CRUD
- Service trait: 7 个方法（create/find_by_id/approve/receive/inspect/complete/reject/list）
- Service impl: 注入 ShippingRequestService（查询发货信息）、DocumentLinkService（REFERENCES）、CostEntryService（complete 时冲减）
- complete: 更新 order_item.returned_qty → 按 disposition 处理 → cost_entry(reverse) → QMS RMA 触发
- create 校验: 发货单 status == Shipped，return_qty <= shipped_qty - already_returned_qty

**Patterns to follow:**
- U3 的跨模块集成模式
- ReturnDisposition 枚举遵循 QuotationStatus 的 #[repr(i16)] 模式

**Test scenarios:**
- Happy path: create with valid Shipped shipping request → status=Draft
- Happy path: approve → Confirmed, receive → Received, inspect → Inspecting, complete → Completed
- Happy path: complete with disposition=Restock → updates order_item.returned_qty
- Happy path: reject Draft → Rejected
- Edge case: create with return_qty > remaining shipped qty → BusinessRule error
- Error path: create with non-Shipped shipping request → BusinessRule error
- Error path: approve non-Draft → state machine rejects
- Integration: complete triggers CostEntry(reverse) + updates order_item.returned_qty

**Verification:**
- `cargo clippy -p abt-core` 通过
- 状态流转完整：Draft → Confirmed → Received → Inspecting → Completed
- return_qty 校验正确（<= shipped_qty - already_returned_qty）

---

### U5. Reconciliation 月对账

**Goal:** 实现月对账模块，含自动聚合发货明细、客户确认、异议处理、结算核销

**Requirements:** R5, R6, R7, R8

**Dependencies:** U3

**Files:**
- Create: `abt-core/migrations/006_create_reconciliation.sql`
- Modify: `abt-core/src/sales/reconciliation/model.rs`
- Modify: `abt-core/src/sales/reconciliation/repo.rs`
- Modify: `abt-core/src/sales/reconciliation/service.rs`
- Modify: `abt-core/src/sales/reconciliation/implt/mod.rs`
- Modify: `abt-core/src/sales/reconciliation/mod.rs`

**Approach:**
- Migration: `reconciliations` 主表（含 customer_id + period UNIQUE 约束）+ `reconciliation_items` 明细表（含 confirmed bool、shipping_request_id FK）
- Model: Reconciliation/ReconciliationItem、ReconciliationStatus（5 个状态）、CreateReconciliationReq
- Repo: 标准 CRUD + find_by_customer_period（唯一约束查询）
- Service trait: 8 个方法（create/find_by_id/send/confirm/dispute/reopen/force_settle/settle/list）
- Service impl: 注入 ShippingRequestService（聚合发货明细）、DocumentLinkService（RECONCILES）、CostEntryService（settle 时 AR voucher）
- create: 校验 customer_id + period 唯一 → 自动聚合该期间 Shipped 发货明细 → 批量创建 items + doc_links
- confirm: 校验所有 item.confirmed == true
- settle: cost_entry(AR voucher) → FMS CashJournal + WriteOff 集成点

**Patterns to follow:**
- U3/U4 的跨模块集成模式
- 唯一约束校验：先查 find_by_customer_period，不存在则创建

**Test scenarios:**
- Happy path: create for customer+period → auto-aggregates shipping items, status=Draft
- Happy path: send → Sent, confirm → Confirmed, settle → Settled
- Happy path: dispute Sent/Confirmed → Disputed, reopen → Draft
- Happy path: force_settle Disputed → Settled
- Edge case: create with duplicate customer_id + period → Duplicate error
- Edge case: create with no shipped items in period → empty reconciliation
- Error path: confirm with unconfirmed items → BusinessRule error
- Error path: settle non-Confirmed → state machine rejects
- Integration: create queries ShippingRequestService for period shipping data
- Integration: settle triggers CostEntryService

**Verification:**
- `cargo clippy -p abt-core` 通过
- customer_id + period 唯一约束正确
- 自动聚合发货明细逻辑完整
- 状态流转覆盖所有路径：Draft→Sent→Confirmed→Settled、Sent/Confirmed→Disputed→Draft(Settled)

---

## System-Wide Impact

- **Interaction graph:** SalesOrderService.confirm → InventoryReservationService.reserve(); ShippingRequestService.ship → InventoryReservationService.fulfill() + CostEntryService.create_entries() + EventBus.publish()
- **Error propagation:** 所有错误统一为 DomainError，repo 层 sqlx::Error 通过 DomainError::Internal 转换，业务校验使用 DomainError::BusinessRule/Validation
- **State lifecycle risks:** 跨表更新（如 ship 时更新 order_item.shipped_qty + shipping_request.status）需在同一事务内，通过 ServiceContext.executor 保证
- **API surface parity:** 5 个 Service trait 接口签名严格匹配 01-sales.html 设计
- **Unchanged invariants:** shared/ 层的任何接口和模型不变；MasterData 的 Service trait 不变

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 跨模块调用可能违反架构规则（sales 直接访问 wms repo） | 严格遵守通过 Service trait 调用，禁止跨模块 repo 访问 |
| DomainEventType enum 可能缺少销售模块需要的事件类型 | 实现时发现缺失则添加到 shared/enums/event.rs |
| 状态机转换规则未配置 | 各子模块的 transition 调用依赖 state_transition_defs 表已有配置，否则 transition() 返回错误 |
| 大量代码同时新增可能引入类型错误 | 每个子模块完成后运行 `cargo clippy -p abt-core` 验证 |

---

## Sources & References

- **Origin document:** [2026-05-24-sales-module-design.md](../superpowers/specs/2026-05-24-sales-module-design.md)
- **Design authority:** [01-sales.html](../../docs/uml-design/01-sales.html)
- **Shared infrastructure spec:** [README.md](../../docs/uml-design/README.md)
- **Reference implementation:** `abt-core/src/master_data/customer/implt/mod.rs`
