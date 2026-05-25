---
title: "feat: Implement Outsourcing Management Module (OM)"
type: feat
status: active
date: 2026-05-25
origin: docs/uml-design/05-outsourcing.html
---

# feat: Implement Outsourcing Management Module (OM)

## Summary

在 `abt-core` 中实现完整的委外管理模块，包含委外单（OutsourcingOrder）CRUD 与状态流转、发料明细（OutsourcingMaterial）管理、快递式追踪节点（OutsourcingTracking），全量集成 6 项共享基础设施服务。gRPC 层通过新增 `abt-core` 依赖桥接到 `abt-grpc`，作为第一个使用新架构的 handler 入口。

---

## Problem Frame

ABT 系统需要管理将生产工序或物料发给外部供应商加工的完整生命周期——从发料、追踪、收货到结算。当前系统缺少委外管理能力，无法追踪外包资产在途状态，也无法将委外转为内部生产。设计文档 `docs/uml-design/05-outsourcing.html` 定义了完整的 UML 类图、状态机、服务接口和跨模块交互契约。

---

## Requirements

- R1. 委外单 CRUD：创建、修改（仅 DRAFT）、查询（单条+分页列表），支持全委外/工序委外/材料委外/委外返工四种类型
- R2. 委外单状态流转：DRAFT→SENT→RECEIVED→CLOSED 核心路径，IN_PRODUCTION/DELIVERED 可选中间态，CONVERTED_TO_INTERNAL 和 CANCELLED 分支路径
- R3. 乐观锁并发控制：所有写操作需传 `expected_version`，冲突返回 `DomainError::ConcurrentConflict`
- R4. 发料明细管理：创建时同步写入材料明细，更新时全量替换（仅 DRAFT），追踪 sent_qty/returned_qty
- R5. 快递式追踪节点：7 个有序节点类型，`record_node` 强制顺序校验，支持 SLA 计划时间和超时查询
- R6. 共享基础设施集成：StateMachineService（状态转换）、AuditLogService（审计日志）、DomainEventBus（领域事件）、DocumentSequenceService（编号生成 OO-2026-05-xxxxx）、DocumentLinkService（单据关联）、CostEntryService（外协成本归集）
- R7. 转自制流程：DRAFT 或 SENT 状态的 FULL/PROCESS 类型委外单可转为内部工单，同步创建 MES 工单并回调材料
- R8. gRPC API：完整暴露 OutsourcingOrderService 和 OutsourcingTrackingService 的所有方法

---

## Scope Boundaries

- 前端交互设计（CLAUDE.md 禁止修改前端代码）
- 旧 `abt` crate 的迁移或修改（新功能一律在 `abt-core` 中开发）
- 超时自动预警的 Worker 定时扫描（设计文档标注"后续可扩展"，当前仅提供 `list_overdue` 查询接口）
- BOM 节点自动拆分逻辑（属 MES.BomService 职责）
- 虚拟库位的创建/管理（由 WMS 模块负责，OM 只引用 `virtual_warehouse_id`）
- WMS.InventoryTransferService 真实调用（当前使用 stub，待 WMS 模块完善后替换）

### Deferred to Follow-Up Work

- WMS 库存调拨真实调用替换 stub：待 WMS.InventoryTransferService 实现后替换
- MES 工单创建真实调用替换 stub：待 MES.WorkOrderService 完善后替换
- QMS IQC 检验触发真实调用替换 stub：待 QMS 模块实现后替换
- 超时预警 Worker 定时扫描：后续迭代扩展

---

## Context & Research

### Relevant Code and Patterns

- **采购模块** (`abt-core/src/purchase/`) — 最佳参考模板，展示完整的 Model→Repo→Service trait→ServiceImpl→gRPC handler 分层
- **采购订单实现** (`abt-core/src/purchase/order/`) — 共享基础设施集成范例（StateMachineService.transition、AuditLogService.record、DomainEventBus.publish）
- **采购枚举** (`abt-core/src/purchase/enums.rs`) — `#[repr(i16)]` + `impl_sqlx_traits!` + `impl_serde_traits!` 宏模式
- **WMS Stubs** (`abt-core/src/wms/stubs.rs`) — 跨模块依赖 stub 模式（WorkOrderStub、QualityGateStub、CostEntryStub）
- **共享基础设施** (`abt-core/src/shared/`) — StateMachineService、AuditLogService、DomainEventBus、DocumentSequenceService、DocumentLinkService、CostEntryService 接口定义
- **gRPC Handler** (`abt-grpc/src/handlers/sales_order.rs`) — Proto↔Model 转换、事务管理、错误映射模式

### Institutional Learnings

- **并发控制**：使用 `SELECT ... FOR UPDATE` 锁定行防止"先读后写"竞态条件（来源：`docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`）
- **错误处理分层**：Handler 层用 `business_error()` 静默返回业务验证错误，基础设施错误用 `err_to_status()` 记录完整日志（来源：`docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`）
- **迁移安全**：使用 `INSERT ... ON CONFLICT DO NOTHING` 保留现有数据，避免 `TRUNCATE`（来源：`docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`）

### External References

- 无外部研究（代码库中有充足的内模式参考）

---

## Key Technical Decisions

- **version(i32) 乐观锁**：设计文档明确使用 `version: i32` 字段（非 `updated_at`），UPDATE 时 `WHERE version = $expected_version`，affected_rows==0 则返回 `ConcurrentConflict`。与采购模块的 `updated_at` 乐观锁不同，这是设计文档的明确选择
- **跨模块调用使用 stub**：WMS/MES/QMS 的跨模块服务（InventoryTransferService、WorkOrderService、InspectionResultService）当前使用 stub 实现，保证 OM 模块可独立编译运行，后续真实实现后替换。stub 放在 `abt-core/src/om/stubs.rs` 中
- **子表无独立 version**：OutsourcingMaterial 和 OutsourcingTracking 不设独立 version 字段，依赖父表 OutsourcingOrder 的 version 校验在事务内自然串行化（设计文档明确）
- **abt-grpc 新增 abt-core 依赖**：OM handler 是第一个使用 `abt-core` 的 gRPC handler。`AppState` 需扩展以持有 `abt-core` 的 PgPool 和服务实例
- **追踪节点为可选增强**：追踪节点不影响核心状态流（SENT 可直接到 RECEIVED），`record_node` 中追踪节点副作用触发的状态变更（如 SUPPLIER_RECEIVED→IN_PRODUCTION）作为附加逻辑实现

---

## Open Questions

### Resolved During Planning

- **乐观锁字段**：设计文档明确为 `version: i32`，非 `updated_at`
- **DomainEventType 缺失**：需要新增 `OutsourcingCancelled`（设计文档提到但枚举缺失），下一个可用编号为 47
- **CostEntityType 缺失**：需要新增 `OutsourcingOrder` 变体（编号 6），用于成本归集

### Deferred to Implementation

- 精确的 SQL 查询细节（动态条件 WHERE 子句）—— 实现时参照采购模块的动态查询模式
- 虚拟库位的具体 ID 传递方式 —— 假设由前端/WMS 模块提供，OM 只存储引用
- `convert_to_internal` 创建的 MES 工单的具体字段映射 —— 实现时根据 MES.WorkOrderService.create 的参数确定

---

## Output Structure

```
abt-core/src/om/
├── mod.rs                          # 模块声明 + pub use 重新导出
├── enums.rs                        # OutsourcingType, OutsourcingStatus, TrackingNodeType
├── stubs.rs                        # 跨模块依赖 stub
├── outsourcing_order/
│   ├── mod.rs                      # 子模块声明
│   ├── model.rs                    # OutsourcingOrder + OutsourcingMaterial 实体 + 请求/查询结构体
│   ├── repo.rs                     # OutsourcingOrderRepo + OutsourcingMaterialRepo
│   ├── service.rs                  # OutsourcingOrderService trait
│   └── implt/
│       └── mod.rs                  # OutsourcingOrderServiceImpl
├── outsourcing_tracking/
│   ├── mod.rs                      # 子模块声明
│   ├── model.rs                    # OutsourcingTracking 实体 + 请求/查询结构体
│   ├── repo.rs                     # OutsourcingTrackingRepo
│   ├── service.rs                  # OutsourcingTrackingService trait
│   └── implt/
│       └── mod.rs                  # OutsourcingTrackingServiceImpl

proto/abt/v1/
└── outsourcing.proto               # gRPC 消息和服务定义

abt-grpc/src/
├── handlers/
│   └── outsourcing.rs              # OutsourcingOrderHandler + OutsourcingTrackingHandler

abt-core/migrations/
└── XXX_create_outsourcing_tables.sql
```

---

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

### 状态流转

```
DRAFT ──send()──→ SENT ──receive()──→ RECEIVED ──CLOSED──→ CLOSED
  │                 │
  │                 ├──(SUPPLIER_RECEIVED 节点)──→ IN_PRODUCTION
  │                 │                                    │
  │                 │                 (SHIPPED 节点)──→ DELIVERED
  │                 │                                    │
  │                 └────────────────────receive()──→ RECEIVED
  │
  ├──cancel()──→ CANCELLED
  │
  └──convert_to_internal()──→ CONVERTED_TO_INTERNAL
         (DRAFT 或 SENT)
```

### 写操作事务模式

`send()`/`receive()`/`convert_to_internal()`/`cancel()` 内部步骤在同一数据库事务内完成：

1. 校验乐观锁 version
2. StateMachineService.transition()（同步强一致）
3. 更新实体表状态 + 业务字段
4. 跨模块调用（WMS/MES/QMS — 同步强一致，stub 时为空操作）
5. OutsourcingTrackingService.record_node()（同步强一致）
6. AuditLogService.record()（同步强一致）
7. DomainEventBus.publish()（Outbox 模式，写 Outbox + NOTIFY）
8. DocumentLinkService.create_links()（异步 Outbox）

CostEntryService 使用独立事务模式：主事务提交后开新事务记录成本，失败不影响主业务。

---

## Implementation Units

### U1. Proto 定义 + 构建集成

**Goal:** 创建 `outsourcing.proto` 定义所有 gRPC 消息和服务接口，确保 `cargo build` 自动生成代码

**Requirements:** R1, R2, R5, R8

**Dependencies:** None

**Files:**
- Create: `proto/abt/v1/outsourcing.proto`
- Modify: `abt-grpc/build.rs`（如需注册新 proto 文件）

**Approach:**
- 定义 OutsourcingType、OutsourcingStatus、TrackingNodeType 三个枚举（Proto 层大写下划线命名，如 OUTSOURCING_TYPE_FULL）
- 定义 OutsourcingOrder、OutsourcingMaterial、OutsourcingTracking 消息
- 定义所有请求/响应消息（CreateOutsourcingOrderRequest、SendOutsourcingRequest 等）
- 定义 OutsourcingOrderService 和 OutsourcingTrackingService 两个 gRPC service
- 复用 proto/abt/v1/base.proto 中的 U64Response、BoolResponse、PaginationInfo
- Decimal 字段使用 string 表示（与现有 proto 一致）
- 时间字段使用 int64 timestamp（与现有 proto 一致）

**Patterns to follow:**
- `proto/abt/v1/sales_order.proto` — 消息命名、字段风格
- `proto/abt/v1/quotation.proto` — 分页查询模式

**Test scenarios:**
- Test expectation: none -- Proto 定义是纯 IDL，通过 `cargo build` 验证编译和代码生成

**Verification:**
- `cargo build` 成功生成 `abt-grpc/src/generated/` 中的 Rust 代码
- 生成的代码包含 OutsourcingOrderService 和 OutsourcingTrackingService server trait

---

### U2. 枚举定义 + 共享枚举扩展

**Goal:** 创建 OM 模块专属枚举并扩展共享枚举（DomainEventType、CostEntityType）

**Requirements:** R1, R2, R5, R6

**Dependencies:** None

**Files:**
- Create: `abt-core/src/om/enums.rs`
- Modify: `abt-core/src/shared/enums/event.rs`（新增 OutsourcingCancelled = 47）
- Modify: `abt-core/src/shared/enums/cost.rs`（新增 OutsourcingOrder = 6）

**Approach:**
- OutsourcingType: FULL=1, PROCESS=2, MATERIAL=3, REWORK=4
- OutsourcingStatus: DRAFT=1, SENT=2, IN_PRODUCTION=3, DELIVERED=4, RECEIVED=5, CLOSED=6, CONVERTED_TO_INTERNAL=7, CANCELLED=8
- TrackingNodeType: SEND_MATERIAL=0, CARRIER_PICKUP=1, SUPPLIER_RECEIVED=2, IN_PRODUCTION=3, SHIPPED=4, IQC_INSPECTED=5, WAREHOUSED=6
- 所有枚举使用 `#[repr(i16)]`，实现 `from_i16/as_i16/as_str` 方法
- 复用 `purchase/enums.rs` 中的 `impl_sqlx_traits!` 和 `impl_serde_traits!` 宏
- TrackingNodeType 额外实现 `ordinal()` 方法返回序号（用于节点顺序校验）

**Patterns to follow:**
- `abt-core/src/purchase/enums.rs` — 枚举定义 + 宏模式

**Test scenarios:**
- Happy path: 每个 enum 的 from_i16/as_i16 往返转换正确
- Edge case: 未知 i16 值返回 None
- Happy path: TrackingNodeType::SEND_MATERIAL.ordinal() == 0, WAREHOUSED.ordinal() == 6

**Verification:**
- `cargo clippy -p abt-core` 通过
- 所有枚举的 from_i16 ↔ as_i16 往返测试通过

---

### U3. 数据库迁移

**Goal:** 创建 outsourcing_orders、outsourcing_materials、outsourcing_trackings 三张表

**Requirements:** R1, R3, R4, R5

**Dependencies:** U2（枚举值的 SMALLINT 映射）

**Files:**
- Create: `abt-core/migrations/XXX_create_outsourcing_tables.sql`

**Approach:**
- 参照现有迁移编号规则确定文件名编号
- `outsourcing_orders`: id, doc_number, work_order_id, routing_id, supplier_id, product_id, outsourcing_type(SMALLINT), planned_qty(NUMERIC(18,6)), completed_qty(NUMERIC(18,6)), unit_price(NUMERIC(18,6)), scheduled_date(DATE), status(SMALLINT), virtual_warehouse_id, version(INT DEFAULT 1), remark(TEXT), operator_id, created_at, updated_at, deleted_at
- `outsourcing_materials`: id, outsourcing_id, product_id, planned_qty, sent_qty, returned_qty, unit_cost(NUMERIC(18,6))
- `outsourcing_trackings`: id, outsourcing_id, node_type(SMALLINT), tracked_at(TIMESTAMPTZ), planned_at(TIMESTAMPTZ), remark(TEXT), operator_id, created_at(TIMESTAMPTZ)
- 无外键约束（应用层强制）
- 标准 created_at/updated_at 默认值
- deleted_at 实现软删除（仅 outsourcing_orders）
- 适当的索引：outsourcing_orders(doc_number), outsourcing_orders(status), outsourcing_orders(supplier_id), outsourcing_materials(outsourcing_id), outsourcing_trackings(outsourcing_id, node_type)

**Patterns to follow:**
- 现有 `abt-core/migrations/` 中的 SQL 文件

**Test scenarios:**
- Test expectation: none -- 迁移通过 SQL 执行验证，后续单元测试验证 CRUD

**Verification:**
- 迁移文件 SQL 语法正确（PostgreSQL 兼容）
- 表结构与设计文档中的实体定义一一对应

---

### U4. Models + Repositories

**Goal:** 创建 OutsourcingOrder、OutsourcingMaterial、OutsourcingTracking 的 Rust 结构体和数据库访问层

**Requirements:** R1, R3, R4, R5

**Dependencies:** U2（枚举类型）, U3（数据库表）

**Files:**
- Create: `abt-core/src/om/outsourcing_order/model.rs`
- Create: `abt-core/src/om/outsourcing_order/repo.rs`
- Create: `abt-core/src/om/outsourcing_tracking/model.rs`
- Create: `abt-core/src/om/outsourcing_tracking/repo.rs`

**Approach:**

**Model:**
- OutsourcingOrder: `#[derive(Debug, Clone, sqlx::FromRow)]`，字段映射数据库行
- OutsourcingMaterial: 同上，含计算辅助方法 `in_transit_qty = sent_qty - returned_qty`
- OutsourcingTracking: 同上
- 请求结构体：CreateOutsourcingOrderReq、UpdateOutsourcingOrderReq、SendOutsourcingReq、ReceiveOutsourcingReq、ConvertToInternalReq、CancelOutsourcingReq、OutsourcingMaterialItem
- 查询结构体：OutsourcingOrderQuery、OverdueTrackingQuery
- 所有请求结构体与设计文档中定义一致

**Repository:**
- OutsourcingOrderRepo: insert, get_by_id, update, update_status_and_version, query（分页+动态条件）
- OutsourcingMaterialRepo: insert_batch, list_by_outsourcing_id, update_sent_qty, update_returned_qty, replace_batch（全量替换）
- OutsourcingTrackingRepo: insert, list_by_outsourcing_id, get_max_node_ordinal, query_overdue
- 所有 Repo 使用静态方法，返回 `Result<T, sqlx::Error>`
- 乐观锁：`UPDATE ... SET version = version + 1 WHERE id = $1 AND version = $2`
- 动态条件查询：`WHERE ($1 IS NULL OR col = $1)` 模式
- 分页查询返回 `(Vec<T>, u64)`

**Patterns to follow:**
- `abt-core/src/purchase/order/model.rs` — Model 结构体模式
- `abt-core/src/purchase/order/repo.rs` — Repo 静态方法、动态查询、乐观锁模式

**Test scenarios:**
- Happy path: OutsourcingOrderRepo.insert → get_by_id 往返数据一致
- Happy path: OutsourcingMaterialRepo.insert_batch → list_by_outsourcing_id 返回正确明细
- Edge case: update_status_and_version 版本不匹配时 affected_rows == 0
- Edge case: 动态条件查询全部为 NULL 时返回全部记录
- Happy path: OutsourcingTrackingRepo.get_max_node_ordinal 返回已完成节点的最大序号
- Edge case: 无追踪节点时 get_max_node_ordinal 返回 -1 或 None
- Happy path: OutsourcingTrackingRepo.query_overdue 正确筛选 planned_at < now() AND tracked_at IS NULL

**Verification:**
- `cargo clippy -p abt-core` 通过
- 单元测试覆盖 CRUD 和乐观锁冲突场景

---

### U5. Service Traits

**Goal:** 定义 OutsourcingOrderService 和 OutsourcingTrackingService 的 async trait 接口

**Requirements:** R1, R2, R5, R6, R7, R8

**Dependencies:** U4（Model 类型定义）

**Files:**
- Create: `abt-core/src/om/outsourcing_order/service.rs`
- Create: `abt-core/src/om/outsourcing_tracking/service.rs`
- Create: `abt-core/src/om/stubs.rs`
- Modify: `abt-core/src/om/mod.rs`（模块声明 + pub use 重新导出）

**Approach:**

**OutsourcingOrderService**（8 个方法）:
- create(ctx, req, idempotency_key) → Result<i64>
- update(ctx, req) → Result<()>（仅 DRAFT）
- send(ctx, req) → Result<()>
- receive(ctx, req) → Result<()>
- convert_to_internal(ctx, req) → Result<i64>（返回新工单 ID）
- cancel(ctx, req) → Result<()>（仅 DRAFT）
- find_by_id(ctx, id) → Result<OutsourcingOrder>
- list(ctx, filter, page) → Result<PaginatedResult<OutsourcingOrder>>

**OutsourcingTrackingService**（3 个方法）:
- record_node(ctx, req) → Result<i64>（含顺序校验）
- list_by_outsourcing(ctx, outsourcing_id, page) → Result<PaginatedResult<OutsourcingTracking>>
- list_overdue(ctx, filter, page) → Result<PaginatedResult<OutsourcingTracking>>

**Stubs:**
- WorkOrderStub：get_info, get_bom_components
- InventoryTransferStub：transfer_to_virtual, transfer_from_virtual
- QualityGateStub：is_passed
- SupplierStub：get（引用 master_data 中的 SupplierService）

**Patterns to follow:**
- `abt-core/src/purchase/order/service.rs` — trait 定义模式
- `abt-core/src/wms/stubs.rs` — stub 模式

**Test scenarios:**
- Test expectation: none — trait 定义是纯接口，通过 impl 单元测试验证

**Verification:**
- `cargo clippy -p abt-core` 通过
- 模块导出路径正确（`abt_core::om::OutsourcingOrderService`）

---

### U6. OutsourcingOrderService 实现

**Goal:** 实现 OutsourcingOrderServiceImpl，包含完整的状态流转、共享基础设施集成和跨模块调用

**Requirements:** R1, R2, R3, R4, R6, R7

**Dependencies:** U2, U4, U5

**Files:**
- Create: `abt-core/src/om/outsourcing_order/implt/mod.rs`
- Modify: `abt-core/src/om/outsourcing_order/mod.rs`（子模块声明）
- Modify: `abt-core/src/om/mod.rs`（工厂函数）

**Approach:**

**ServiceImpl 构造函数注入**：doc_seq, state_machine, event_bus, audit_log, doc_link, cost_entry, idempotency

**create():**
1. IdempotencyService 检查
2. DocumentSequenceService.next_number()（DocumentType::OutsourcingOrder，前缀 "OO"）
3. OutsourcingOrderRepo.insert（status=DRAFT, version=1）
4. OutsourcingMaterialRepo.insert_batch
5. AuditLogService.record(AuditAction::Create)
6. 返回 id

**update():** 仅 DRAFT
1. get_by_id + 校验 status == DRAFT
2. 校验 version == expected_version
3. OutsourcingOrderRepo.update
4. 如 materials 存在，OutsourcingMaterialRepo.replace_batch
5. AuditLogService.record(AuditAction::Update)

**send():** DRAFT→SENT
1. get_by_id + 校验 version
2. StateMachineService.transition("Sent")
3. OutsourcingOrderRepo.update_status_and_version
4. InventoryTransferStub.transfer_to_virtual（stub）
5. OutsourcingTrackingService.record_node(SEND_MATERIAL)（委托调用）
6. CostEntryService.create_entries（独立事务模式，借:在制品/贷:应付外协费）
7. AuditLogService.record(AuditAction::Transition)
8. DomainEventBus.publish(OutsourcingSent)
9. DocumentLinkService（OutsourcingOrder→WorkOrder，异步 Outbox）

**receive():** SENT/IN_PRODUCTION/DELIVERED→RECEIVED
1. get_by_id + 校验 version
2. StateMachineService.transition("Received")
3. OutsourcingOrderRepo.update（status + completed_qty + version）
4. InventoryTransferStub.transfer_from_virtual（stub）
5. OutsourcingTrackingService.record_node(WAREHOUSED)
6. QualityGateStub.is_passed → QMS IQC（stub）
7. AuditLogService.record(AuditAction::Transition)
8. DomainEventBus.publish(OutsourcingReceived)

**convert_to_internal():** DRAFT/SENT→CONVERTED_TO_INTERNAL
1. get_by_id + 校验 version + 校验 outsourcing_type ∈ {FULL, PROCESS}
2. StateMachineService.transition("ConvertedToInternal")
3. OutsourcingOrderRepo.update_status_and_version
4. WorkOrderStub.get_info → MES.WorkOrderService.create（stub，返回虚拟工单 ID）
5. InventoryTransferStub.transfer_from_virtual（材料回调）
6. AuditLogService.record(AuditAction::Transition)
7. DomainEventBus.publish(ConvertedToInternal)
8. 返回新工单 ID

**cancel():** DRAFT→CANCELLED
1. get_by_id + 校验 version + 校验 status == DRAFT
2. StateMachineService.transition("Cancelled")
3. OutsourcingOrderRepo.update_status_and_version
4. AuditLogService.record(AuditAction::Transition)
5. DomainEventBus.publish(OutsourcingCancelled)

**Patterns to follow:**
- `abt-core/src/purchase/order/implt/mod.rs` — ServiceImpl 构造函数、共享服务注入、写操作流程编排模式

**Test scenarios:**
- Happy path: create 创建委外单，返回有效 id，status == DRAFT, version == 1
- Happy path: update 修改 DRAFT 状态委外单，材料全量替换成功
- Happy path: send 从 DRAFT→SENT，触发追踪节点、审计、事件
- Happy path: receive 从 SENT→RECEIVED，completed_qty 正确累加
- Happy path: convert_to_internal 从 DRAFT→CONVERTED_TO_INTERNAL，返回新工单 ID
- Happy path: cancel 从 DRAFT→CANCELLED
- Error path: update 非 DRAFT 状态委外单返回 InvalidStateTransition
- Error path: send 时 version 不匹配返回 ConcurrentConflict
- Error path: convert_to_internal 对 MATERIAL/REWORK 类型返回 BusinessRule 错误
- Edge case: send 时无 materials 也能成功发送
- Integration: send 完整流程验证（状态转换 + 追踪节点 + 审计 + 事件 + 文档关联）

**Verification:**
- `cargo clippy -p abt-core` 通过
- 单元测试覆盖所有状态转换路径和错误场景
- 共享基础设施集成点在测试中通过 mock 或 in-memory 验证

---

### U7. OutsourcingTrackingService 实现

**Goal:** 实现 OutsourcingTrackingServiceImpl，包含节点顺序校验和超时查询

**Requirements:** R5

**Dependencies:** U4, U5

**Files:**
- Create: `abt-core/src/om/outsourcing_tracking/implt/mod.rs`
- Modify: `abt-core/src/om/outsourcing_tracking/mod.rs`（子模块声明）

**Approach:**

**record_node():**
1. 获取该委外单已完成的节点列表
2. validate_node_sequence()：目标节点的序号必须 > 已完成节点的最大序号
3. 如果未提供 tracked_at，使用当前时间
4. OutsourcingTrackingRepo.insert
5. 返回追踪节点 id

**validate_node_sequence() 独立函数：**
- 查询已完成节点的最大 ordinal
- 校验 target_ordinal > max_completed_ordinal
- 失败返回 `DomainError::Validation`，包含中文错误消息

**list_by_outsourcing():**
- 按 outsourcing_id 查询，分页返回，按 node_type 排序

**list_overdue():**
- 查询 planned_at < now() AND tracked_at IS NULL 的节点
- 支持按 supplier_id、node_type、overdue_before 筛选

**Patterns to follow:**
- `abt-core/src/purchase/order/implt/mod.rs` — ServiceImpl 模式

**Test scenarios:**
- Happy path: 按顺序录入 SEND_MATERIAL → CARRIER_PICKUP → SUPPLIER_RECEIVED
- Error path: 跳过节点（如 SEND_MATERIAL 后直接录入 WAREHOUSED）返回 Validation 错误
- Edge case: 无已录入节点时，录入任意节点成功
- Happy path: list_by_outsourcing 按序返回节点
- Happy path: list_overdue 正确筛选超时节点（planned_at 在过去且未 tracked）
- Edge case: 无超时节点时 list_overdue 返回空列表

**Verification:**
- `cargo clippy -p abt-core` 通过
- 节点顺序校验测试覆盖所有非法跳转场景

---

### U8. gRPC Handler + abt-grpc 集成

**Goal:** 创建 gRPC handler，注册到 server，完成 abt-grpc 对 abt-core 的桥接

**Requirements:** R8

**Dependencies:** U1（Proto 生成代码）, U6, U7

**Files:**
- Modify: `abt-grpc/Cargo.toml`（新增 abt-core 依赖）
- Create: `abt-grpc/src/handlers/outsourcing.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`（注册新 handler）
- Modify: `abt-grpc/src/server.rs`（AppState 扩展 + handler 注册）

**Approach:**

**AppState 扩展：**
- 新增 `abt_core_pool: Arc<PgPool>` 字段（使用 `ABT_CORE_DATABASE_URL` 环境变量）
- 新增 `outsourcing_order_service()` 和 `outsourcing_tracking_service()` 工厂方法
- 工厂方法内部创建 ServiceImpl 实例（注入共享服务实例）

**Handler 职责：**
- Proto 消息 → Rust 请求结构体的转换
- 构建 ServiceContext（从 gRPC metadata 提取 operator_id、department_id 等）
- 调用 Service trait 方法
- Rust 结果 → Proto 响应的转换
- DomainError → tonic::Status 映射
- 事务管理：写操作需要 `state.begin_transaction()` + `tx.commit()`

**Proto↔Model 转换：**
- OutsourcingType 枚举：Proto enum ↔ Rust enum
- OutsourcingStatus 枚举：Proto enum ↔ Rust enum
- TrackingNodeType 枚举：Proto enum ↔ Rust enum
- Decimal ↔ String（proto 中 string 表示）
- Timestamp ↔ chrono::DateTime
- Date ↔ chrono::NaiveDate

**Patterns to follow:**
- `abt-grpc/src/handlers/sales_order.rs` — handler 模式、事务管理、错误映射
- `abt-grpc/src/server.rs` — AppState 模式

**Test scenarios:**
- Happy path: create_outsourcing_order gRPC 调用返回有效 id
- Happy path: list_outsourcing_orders 返回分页结果
- Happy path: send_outsourcing_order 触发状态变更
- Error path: 无效 id 返回 NOT_FOUND status
- Error path: 版本冲突返回 ABORTED status
- Integration: 完整 DRAFT→SENT→RECEIVED→CLOSED 流程通过 gRPC 调用验证

**Verification:**
- `cargo clippy` 通过
- gRPC reflection 可发现 OutsourcingOrderService 和 OutsourcingTrackingService
- 完整状态流转可通过 gRPC 客户端（如 grpcurl）调用验证

---

## System-Wide Impact

- **Interaction graph:** OM 模块向共享基础设施发出调用（StateMachineService、AuditLogService、DomainEventBus、DocumentSequenceService、DocumentLinkService、CostEntryService）。OM 不直接调用其他业务模块（使用 stub），跨模块交互通过 DomainEventBus 的 Outbox 模式异步解耦
- **Error propagation:** ServiceImpl 层将 sqlx::Error 映射为 DomainError::Internal，gRPC handler 层将 DomainError 映射为 tonic::Status（NotFound→NOT_FOUND, ConcurrentConflict→ABORTED, Validation→INVALID_ARGUMENT, BusinessRule→FAILED_PRECONDITION）
- **State lifecycle risks:** send/receive/convert_to_internal 涉及多步操作在同一事务内，任一步失败整体回滚。CostEntryService 使用独立事务，失败不回滚主事务
- **API surface parity:** Proto 定义是唯一的 API 入口，无 REST/CLI 等其他接口需同步
- **Integration coverage:** 完整状态流转端到端测试验证所有共享基础设施集成点
- **Unchanged invariants:** 现有 purchase/sales/wms/mes 模块不受影响，OM 是纯增量模块

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| abt-grpc 首次引入 abt-core 依赖，AppState 扩展可能影响现有 handler | 新增独立的 abt_core_pool 字段，不修改现有 AppContext 引用 |
| 跨模块 stub 未来替换时需修改 OM impl | stub 接口与真实 Service trait 对齐，替换时仅修改构造函数注入 |
| version 字段乐观锁与 updated_at 乐观锁混用（不同模块不同策略） | OM 模块内部统一使用 version，不与采购模块的 updated_at 策略冲突 |
| TrackingNodeType 序号从 0 开始（非 1），与其他枚举不一致 | 设计文档明确规定，节点顺序校验基于 ordinal 值 |

---

## Sources & References

- **Origin document:** [docs/uml-design/05-outsourcing.html](docs/uml-design/05-outsourcing.html)
- Reference pattern: `abt-core/src/purchase/order/` — 采购模块完整实现
- Reference pattern: `abt-core/src/wms/stubs.rs` — 跨模块 stub 模式
- Shared infrastructure: `abt-core/src/shared/` — 共享服务接口定义
