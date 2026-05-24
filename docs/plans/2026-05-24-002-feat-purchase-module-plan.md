---
title: "feat: Implement Purchase Module (SRM) in abt-core"
type: feat
status: active
date: 2026-05-24
origin: docs/superpowers/specs/2026-05-24-purchase-module-design.md
---

# feat: Implement Purchase Module (SRM) in abt-core

## Summary

在 `abt-core/src/purchase/` 下实现完整的采购模块，包含 6 个子实体（采购报价、采购订单、采购退货、对账单、付款申请、零星请购），12 张数据库表，7 个枚举，6 个 Service trait 及其实现。所有实现严格遵循 `docs/uml-design/02-purchase.html` 设计文档，集成已实现的共享基础设施层。不含 gRPC handler。

---

## Problem Frame

ABT 系统目前缺少采购模块实现。`abt-core/src/purchase/mod.rs` 只有一行占位注释。采购业务（供应商报价比较、订单下达、来料对账、付款申请、退货处理、零星请购）需要完整的后端支撑。共享基础设施层和主数据模块已就绪，为采购模块提供了 DocSeq、AuditLog、EventBus、StateMachine、DocLink 等基础服务。

---

## Requirements

- R1. 创建 12 张数据库表（6 主表 + 6 明细表），遵循现有 migration 格式和约定
- R2. 定义 7 个采购专属枚举（`#[repr(i16)]` + sqlx + serde），遵循 `shared/enums` 模式
- R3. 为每个子实体创建 Model（entity struct + query struct + request struct），使用 `sqlx::FromRow`
- R4. 为每个子实体创建 Repository（insert + get_by_id + query 动态条件分页），遵循 `AuditLogRepo` 模式
- R5. 定义 6 个 `#[async_trait]` Service trait，方法签名严格匹配设计文档
- R6. 实现每个 ServiceImpl，集成共享层服务（DocSeq、AuditLog、EventBus、StateMachine、DocLink）
- R7. PurchaseOrder.confirm() 包含完整的前置校验（供应商状态、行级数据、报价有效期）
- R8. PurchaseReconciliation.confirm() 包含退货冲减公式和关联退货状态驱动
- R9. PaymentRequest 包含三单匹配校验和 SRM/FMS 边界尊重
- R10. 所有写操作使用乐观锁（`WHERE id = ? AND updated_at = ?`），失败抛 `ConcurrentConflict`
- R11. 所有写操作支持 `idempotency_key`
- R12. `abt-core/src/lib.rs` 注册模块并导出工厂函数

---

## Scope Boundaries

- 不包含 gRPC handler 层（proto 定义和 handler 实现后续迭代）
- 不包含前端交互设计
- 不修改 `abt` crate 代码（只在 `abt-core` 中开发）
- 不修改共享基础设施层代码（只集成使用）
- 不修改 `shared/enums` 中已有的 DocumentType 和 DomainEventType（已预留采购相关值）

### Deferred to Follow-Up Work

- Proto 定义 + gRPC handler：单独的后续 PR
- WMS 来料入库事件监听（ArrivalReceived handler）：依赖 WMS 模块实现
- FMS 付款回调事件监听（PaymentExecuted handler）：依赖 FMS 模块实现
- QMS 来料检验硬门检查集成：依赖 QMS 模块实现
- 前端交互设计与实现

---

## Context & Research

### Relevant Code and Patterns

- **Migration 模式**: `abt-core/migrations/001_create_shared_infrastructure.sql` — `BEGIN;` / `COMMIT;` 事务包裹，SMALLINT 存枚举，注释风格
- **枚举模式**: `abt-core/src/shared/enums/document_type.rs` — `#[repr(i16)]` + `from_i16/as_i16` + sqlx Type/Encode/Decode + serde Serialize/Deserialize
- **Model 模式**: `abt-core/src/shared/audit_log/model.rs` — `#[derive(Debug, Clone, sqlx::FromRow)]` struct + `Default` Query struct
- **Repo 模式**: `abt-core/src/shared/audit_log/repo.rs` — 静态方法、动态条件 SQL（`$1::type IS NULL OR col = $1`）、返回 `(Vec<T>, u64)`
- **Service Trait 模式**: `abt-core/src/shared/audit_log/service.rs` — `#[async_trait]` trait + `Send + Sync`、`ServiceContext<'_>` 第一个参数
- **ServiceImpl 模式**: `abt-core/src/shared/audit_log/implt/mod.rs` — `Arc<PgPool>` 构造、`ctx.executor` 事务使用、`DomainError` 错误映射
- **子模块组织**: `abt-core/src/sales/sales_order/mod.rs` — `pub mod implt/model/repo/service` + `pub use service::Trait`

### Institutional Learnings

- 权限系统使用宏拦截（`require-permission-macro-async-trait`），新模块需预留权限点
- 业务错误使用 `DomainError` 而非裸字符串，保证错误可追踪

---

## Key Technical Decisions

- **枚举集中管理**: 采购模块 7 个枚举集中到 `purchase/enums.rs`，而非分散到各子模块 model.rs。参考 `shared/enums` 的文件组织方式，但放在 `purchase` 模块内（shared 层不应依赖业务模块）
- **子模块命名**: `return_order` 而非 `return`（避免 Rust 保留字冲突）
- **分层渐增实现顺序**: Migration → Enums → Models → Repos + Traits → ServiceImpls → Registration。这是用户确认的实现顺序，每层提交验证后再进入下一层
- **ServiceImpl 按实体分两个单元**: U5 处理独立实体（Quotation + MiscRequest），U6 处理核心流转链（Order → Return → Reconciliation → Payment），因为后者之间有业务依赖
- **Decimal 类型**: Rust 侧使用 `rust_decimal::Decimal`，对应 PostgreSQL `NUMERIC(18,6)` / `NUMERIC(20,4)`。参考项目已有约定
- **乐观锁实现**: 所有 update/confirm/approve 操作在 SQL WHERE 中包含 `updated_at = $N`，返回 affected rows = 0 时抛 `DomainError::ConcurrentConflict`

---

## Open Questions

### Resolved During Planning

- **gRPC handler 是否包含**: 不包含，只做 abt-core 层（用户确认）
- **共享层是否集成**: 是，完整集成所有已实现的共享服务（用户确认）
- **Migration 是否包含**: 是，12 张表（用户确认）

### Deferred to Implementation

- **ServiceImpl 中跨模块调用的具体注入方式**: ServiceImpl 构造函数需要接收共享服务的 trait object 或 Arc。具体注入方式（构造函数参数列表、工厂函数签名）在实现时根据现有 `sales` 模块的注册模式确定
- **tolerance_rate 的存储位置**: 三单匹配的容差率配置，实现时决定放在配置表还是硬编码默认值
- **DataScope 过滤的具体 SQL 条件**: 行级权限的 `department_id IN` 条件需要确认 departments 表结构和关联方式

---

## Output Structure

```
abt-core/src/purchase/
├── mod.rs
├── enums.rs
├── quotation/
│   ├── mod.rs
│   ├── model.rs
│   ├── repo.rs
│   ├── service.rs
│   └── implt/
│       └── mod.rs
├── order/
│   ├── mod.rs
│   ├── model.rs
│   ├── repo.rs
│   ├── service.rs
│   └── implt/
│       └── mod.rs
├── return_order/
│   ├── mod.rs
│   ├── model.rs
│   ├── repo.rs
│   ├── service.rs
│   └── implt/
│       └── mod.rs
├── reconciliation/
│   ├── mod.rs
│   ├── model.rs
│   ├── repo.rs
│   ├── service.rs
│   └── implt/
│       └── mod.rs
├── payment/
│   ├── mod.rs
│   ├── model.rs
│   ├── repo.rs
│   ├── service.rs
│   └── implt/
│       └── mod.rs
└── misc_request/
    ├── mod.rs
    ├── model.rs
    ├── repo.rs
    ├── service.rs
    └── implt/
        └── mod.rs

abt-core/migrations/
└── 002_create_purchase_tables.sql
```

---

## Implementation Units

### U1. Database Migration

**Goal:** 创建采购模块的 12 张数据库表（6 主表 + 6 明细表），包含所有约束、索引和状态机配置

**Requirements:** R1

**Dependencies:** None

**Files:**
- Create: `abt-core/migrations/002_create_purchase_tables.sql`

**Approach:**
- 遵循 `001_create_shared_infrastructure.sql` 的格式：`BEGIN;` / `COMMIT;` 事务包裹
- 所有枚举存储为 SMALLINT，应用层强制类型安全
- 金额字段使用 NUMERIC(18,6) 和 NUMERIC(20,4)
- `purchase_reconciliations` 的 `UNIQUE(supplier_id, period)` 使用 partial unique index（`WHERE deleted_at IS NULL`）
- 所有表含 `deleted_at TIMESTAMPTZ` 实现软删除
- 为 `supplier_id`、`status`、`doc_number` 等常用查询字段创建索引
- 表结构严格匹配设计文档 `02-purchase.html`

**Patterns to follow:**
- `abt-core/migrations/001_create_shared_infrastructure.sql` — 格式、命名、索引风格

**Test scenarios:**
- Test expectation: none — migration 文件，通过 `cargo clippy` 和后续 ServiceImpl 集成测试间接验证

**Verification:**
- migration 文件 SQL 语法正确，可通过 `psql` 执行
- 表结构覆盖设计文档中全部 12 张表的全部列和约束

---

### U2. Enums and Module Skeletons

**Goal:** 创建 7 个采购专属枚举和 6 个子模块的目录骨架

**Requirements:** R2

**Dependencies:** None

**Files:**
- Create: `abt-core/src/purchase/enums.rs`
- Create: `abt-core/src/purchase/quotation/mod.rs`
- Create: `abt-core/src/purchase/order/mod.rs`
- Create: `abt-core/src/purchase/return_order/mod.rs`
- Create: `abt-core/src/purchase/reconciliation/mod.rs`
- Create: `abt-core/src/purchase/payment/mod.rs`
- Create: `abt-core/src/purchase/misc_request/mod.rs`
- Modify: `abt-core/src/purchase/mod.rs` — 替换占位注释为模块声明 + `pub use`

**Approach:**
- `enums.rs` 中定义 7 个枚举，每个遵循 `#[repr(i16)]` + `from_i16/as_i16` + sqlx Type/Encode/Decode + serde 模式
- 每个子模块 `mod.rs` 声明 `pub mod implt/model/repo/service`（文件暂时为空或最小占位）
- `purchase/mod.rs` 声明所有子模块和 `pub mod enums`

**Patterns to follow:**
- `abt-core/src/shared/enums/document_type.rs` — 枚举编码模式
- `abt-core/src/sales/sales_order/mod.rs` — 子模块组织模式

**Test scenarios:**
- Happy path: 每个枚举的 `from_i16` / `as_i16` 往返转换正确
- Edge case: `from_i16` 对无效值返回 `None`
- Integration: sqlx 编解码往返（如果测试基础设施支持）

**Verification:**
- `cargo clippy -p abt-core` 通过
- `purchase/mod.rs` 正确导出所有子模块和枚举

---

### U3. Models

**Goal:** 为全部 12 个实体创建 Rust 数据结构（entity + query + create request structs）

**Requirements:** R3

**Dependencies:** U2（枚举定义）

**Files:**
- Create: `abt-core/src/purchase/quotation/model.rs`
- Create: `abt-core/src/purchase/order/model.rs`
- Create: `abt-core/src/purchase/return_order/model.rs`
- Create: `abt-core/src/purchase/reconciliation/model.rs`
- Create: `abt-core/src/purchase/payment/model.rs`
- Create: `abt-core/src/purchase/misc_request/model.rs`

**Approach:**
- 每个子模块的 `model.rs` 包含：
  - 主表 entity struct（`#[derive(Debug, Clone, sqlx::FromRow)]`）
  - 明细表 entity struct
  - Query struct（`#[derive(Debug, Clone, Default)]`，用于动态条件查询）
  - CreateRequest struct（用于 create 方法的入参）
- 字段类型映射：`BIGINT → i64`，`SMALLINT → 枚举`，`NUMERIC → Decimal`，`TIMESTAMPTZ → DateTime<Utc>`，`DATE → NaiveDate`
- 所有 struct 字段使用 `pub` 以便跨层访问

**Patterns to follow:**
- `abt-core/src/shared/audit_log/model.rs` — entity + query struct 模式

**Test scenarios:**
- Test expectation: none — 纯数据结构，无行为逻辑。通过后续 repo 测试间接验证 `FromRow` 映射正确性

**Verification:**
- `cargo clippy -p abt-core` 通过
- 每个 model struct 的字段与 migration 表列一一对应

---

### U4. Repositories and Service Traits

**Goal:** 创建 6 个 Repository（SQL 查询层）和 6 个 Service trait（业务接口定义）

**Requirements:** R4, R5

**Dependencies:** U3（Model 定义）

**Files:**
- Create: `abt-core/src/purchase/quotation/repo.rs`
- Create: `abt-core/src/purchase/quotation/service.rs`
- Create: `abt-core/src/purchase/order/repo.rs`
- Create: `abt-core/src/purchase/order/service.rs`
- Create: `abt-core/src/purchase/return_order/repo.rs`
- Create: `abt-core/src/purchase/return_order/service.rs`
- Create: `abt-core/src/purchase/reconciliation/repo.rs`
- Create: `abt-core/src/purchase/reconciliation/service.rs`
- Create: `abt-core/src/purchase/payment/repo.rs`
- Create: `abt-core/src/purchase/payment/service.rs`
- Create: `abt-core/src/purchase/misc_request/repo.rs`
- Create: `abt-core/src/purchase/misc_request/service.rs`
- Modify: 各子模块 `mod.rs` — 添加 `pub use service::TraitName`

**Approach:**
- 每个 Repository 包含：`insert`、`get_by_id`（含软删除过滤）、`query`（动态条件分页）、`update_status`（乐观锁）
- 每个 Service trait 方法签名严格匹配设计文档，第一个参数为 `ctx: ServiceContext<'_>`
- 写操作返回 `Result<i64, DomainError>`（返回新建 id）或 `Result<(), DomainError>`
- 读操作返回 `Result<Entity, DomainError>` 或 `Result<PaginatedResult<Entity>, DomainError>`
- 各子模块 `mod.rs` 导出 service trait（`pub use service::PurchaseXxxService`）

**Patterns to follow:**
- `abt-core/src/shared/audit_log/repo.rs` — 动态条件 SQL、分页查询模式
- `abt-core/src/shared/audit_log/service.rs` — trait 定义模式
- `abt-core/src/sales/sales_order/mod.rs` — 子模块 pub use 模式

**Test scenarios:**
- Test expectation: none — trait 定义和 SQL 查询，通过后续 ServiceImpl 集成测试验证

**Verification:**
- `cargo clippy -p abt-core` 通过
- 所有 Service trait 方法签名与设计文档一致
- 各 `mod.rs` 正确导出 trait

---

### U5. ServiceImpl — Quotation and MiscellaneousRequest

**Goal:** 实现独立的两个子实体（无跨子实体依赖）：采购报价和零星请购

**Requirements:** R6, R10, R11

**Dependencies:** U4（Service trait 定义）

**Files:**
- Create: `abt-core/src/purchase/quotation/implt/mod.rs`
- Create: `abt-core/src/purchase/misc_request/implt/mod.rs`
- Test: `abt-core/src/purchase/quotation/implt/mod.rs`（内联 `#[cfg(test)]` 模块）
- Test: `abt-core/src/purchase/misc_request/implt/mod.rs`（内联 `#[cfg(test)]` 模块）

**Approach:**
- **PurchaseQuotationServiceImpl**:
  - `create`: 调用 `DocSeq.next_number(PurchaseQuotation)` + insert 主表和明细行 + `AuditLog.record(Create)`
  - `activate`: 状态机 Draft → Active + `AuditLog.record(Transition)` + `EventBus.publish()`
  - `compare`: 查询同一 product_id 下多个供应商的报价，返回 `Vec<QuotationComparison>`
  - `list`/`get`: 标准查询 + DataScope 过滤
- **MiscellaneousRequestServiceImpl**:
  - `create`: 调用 `DocSeq.next_number(MiscellaneousRequest)` + insert + `AuditLog.record(Create)`
  - `approve`: 状态机 Draft → Approved + `EventBus.publish()` + `AuditLog.record(Transition)`
  - `get`: 标准查询
- 构造函数接收 `Arc<PgPool>` + 共享服务 trait objects
- 所有写操作集成 `IdempotencyService`

**Patterns to follow:**
- `abt-core/src/shared/audit_log/implt/mod.rs` — ServiceImpl 结构、构造函数、错误映射

**Test scenarios:**
- Happy path: `create` 成功返回 id，`get` 返回完整实体
- Happy path: `activate` 成功将状态从 Draft 变为 Active
- Happy path: `compare` 对同一 product 返回多个供应商报价
- Edge case: `activate` 对非 Draft 状态的报价返回 `BusinessRule` 错误
- Error path: `create` 对已删除的 supplier_id 时的行为
- Integration: `create` 调用 DocSeq 生成编号 + AuditLog 写入

**Verification:**
- `cargo clippy -p abt-core` 通过
- `create` → `get` 往返验证
- 状态变更通过 StateMachine 校验

---

### U6. ServiceImpl — Order, Return, Reconciliation, Payment

**Goal:** 实现核心采购流转链的 4 个 ServiceImpl，包含完整的业务校验和共享层集成

**Requirements:** R6, R7, R8, R9, R10, R11

**Dependencies:** U5（PurchaseQuotationServiceImpl，因为 Order.create_from_quotation 依赖）

**Files:**
- Create: `abt-core/src/purchase/order/implt/mod.rs`
- Create: `abt-core/src/purchase/return_order/implt/mod.rs`
- Create: `abt-core/src/purchase/reconciliation/implt/mod.rs`
- Create: `abt-core/src/purchase/payment/implt/mod.rs`
- Test: 每个文件内联 `#[cfg(test)]` 模块

**Approach:**
- **PurchaseOrderServiceImpl**:
  - `create`: DocSeq + insert + AuditLog + Idempotency
  - `create_from_quotation`: 从 PQ 读取行项，创建 PO 并建立 DocLink(DERIVED_FROM)
  - `confirm`: 前置校验（SupplierService 校验状态、行级 quantity/price 校验、报价有效期校验）→ StateMachine Draft → Confirmed → EventBus.publish(PurchaseOrderConfirmed) → AuditLog
  - `list`/`get`: 标准查询 + DataScope
- **PurchaseReturnServiceImpl**:
  - `create`: 校验 order 存在且已 Confirmed → DocSeq + insert + DocLink(REFERENCES) + AuditLog
  - `confirm`: StateMachine Draft → Confirmed + EventBus + AuditLog
  - `get`: 标准查询
- **PurchaseReconciliationServiceImpl**:
  - `create(supplier_id, period)`: 校验 same supplier+period 唯一性（partial unique index 保证）→ 汇总该 supplier+period 下所有已入库的 PO 行项 → insert 主表和明细行
  - `confirm`: 汇总公式（应付 = ∑收货 - ∑退货 + 调整）→ 更新 confirmed 标记 → 驱动关联 Return 状态 Shipped → Settled → EventBus 通知 FMS
  - `get`: 标准查询
- **PaymentRequestServiceImpl**:
  - `create`: 三单匹配校验（PO.received_qty vs invoice qty/amount，tolerance_rate）→ DocSeq + insert + AuditLog
  - `approve`: StateMachine Draft → Approved + CostEntry(cash outflow) + EventBus + AuditLog
  - `mark_paid_by_fms`: StateMachine Approved → Paid（FMS 回调边界）
  - `get`: 标准查询
- 所有写操作使用乐观锁 + IdempotencyService

**Patterns to follow:**
- `abt-core/src/shared/audit_log/implt/mod.rs` — ServiceImpl 结构
- `abt-core/src/shared/audit_log/service.rs` — 共享服务调用模式

**Test scenarios:**
- Happy path: Order `create_from_quotation` 从已有报价创建订单
- Happy path: Order `confirm` 通过所有前置校验
- Edge case: Order `confirm` 对 Blacklisted 供应商返回 `BusinessRule` 错误
- Edge case: Order `confirm` 对 quantity=0 的行项返回 `Validation` 错误
- Edge case: Order `confirm` 对过期报价返回 `BusinessRule` 错误
- Happy path: Reconciliation `create` 自动汇总入库明细
- Happy path: Reconciliation `confirm` 正确计算退货冲减
- Integration: Reconciliation `confirm` 驱动 Return 状态变更
- Happy path: Payment `create` 通过三单匹配校验
- Edge case: Payment `create` 对不匹配的发票数量返回 `BusinessRule` 错误
- Happy path: Payment `mark_paid_by_fms` 将状态更新为 Paid
- Error path: 乐观锁冲突返回 `ConcurrentConflict`

**Verification:**
- `cargo clippy -p abt-core` 通过
- `create_from_quotation` → `confirm` 全流程通过
- Reconciliation 退货冲减公式计算正确
- Payment 三单匹配逻辑在容差范围内通过/拒绝

---

### U7. Module Registration and Factory Functions

**Goal:** 在 `lib.rs` 中注册采购模块，创建工厂函数，更新各子模块 `mod.rs` 的 pub use 导出

**Requirements:** R12

**Dependencies:** U6（所有 ServiceImpl 完成）

**Files:**
- Modify: `abt-core/src/purchase/mod.rs` — 完善模块声明和 re-exports
- Modify: `abt-core/src/lib.rs` — 确认 `pub mod purchase` 已声明（当前已存在）
- Modify: 各子模块 `mod.rs` — 添加 `pub use model::*` 和 `pub use service::TraitName`

**Approach:**
- `purchase/mod.rs` 导出所有公开类型（enums、service traits、models）
- 工厂函数模式参照 `sales` 模块（如果已有）或在 `lib.rs` 中新增
- 确认 `pub mod purchase` 在 `lib.rs` 中已声明

**Patterns to follow:**
- `abt-core/src/sales/mod.rs` — 模块注册模式
- `abt-core/src/lib.rs` — 模块声明模式

**Test scenarios:**
- Happy path: `use abt_core::purchase::PurchaseOrderService` 编译通过
- Happy path: `use abt_core::purchase::enums::PurchaseOrderStatus` 编译通过

**Verification:**
- `cargo clippy -p abt-core` 通过
- 外部 crate 可以通过 `abt_core::purchase::*` 访问所有公开类型

---

## System-Wide Impact

- **Interaction graph:** PurchaseOrder.confirm → EventBus(PurchaseOrderConfirmed) → Outbox → WMS ArrivalNotice（异步，WMS 模块需订阅）。PaymentRequest.mark_paid_by_fms ← FMS PaymentExecuted 事件（异步回调）
- **Error propagation:** 所有 ServiceImpl 返回 `Result<T, DomainError>`。Repository 层 `sqlx::Error` 通过 `DomainError::Internal` 包装。乐观锁冲突 → `DomainError::ConcurrentConflict`
- **State lifecycle risks:** PurchaseReconciliation.confirm 会驱动关联 PurchaseReturn 状态变更（Shipped → Settled），需在同一个事务或 Outbox 事件中保证一致性
- **API surface parity:** gRPC handler 层后续实现时，需要将 `DomainError` 映射为 `tonic::Status`（参照 `sales` handler 模式）
- **Integration coverage:** StateMachine 集成、DocSeq 编号生成、AuditLog 写入需在 ServiceImpl 集成测试中验证
- **Unchanged invariants:** shared 层接口签名不变。DocumentType 枚举中已预留的采购相关值（PurchaseQuotation=6, PurchaseOrder=7, etc.）不变

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| ServiceImpl 构造函数依赖多个共享服务 trait，参数列表可能过长 | 使用 builder 模式或共享服务容器 struct |
| 三单匹配 tolerance_rate 配置缺少存储位置 | 初始实现使用硬编码默认值（±0.5%），后续迁移到配置表 |
| Reconciliation.confirm 驱动 Return 状态变更的事务一致性 | 使用同事务内 StateMachine.transition 或 Outbox 事件保证 |
| 跨模块 SupplierService 调用的注入方式 | ServiceImpl 构造函数接收 `Arc<dyn SupplierService>` trait object |

---

## Documentation / Operational Notes

- migration 文件编号为 `002`，在 shared infrastructure migration 之后
- 状态机转换规则（state_transition_defs 表）在 migration 中初始化，后续可通过管理界面配置
- 单据编号格式已在 DocumentType.prefix() 中定义：PQ/PO/PRT/PAY/MISC + REC（对账）

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-24-purchase-module-design.md](docs/superpowers/specs/2026-05-24-purchase-module-design.md)
- **Design document:** [docs/uml-design/02-purchase.html](docs/uml-design/02-purchase.html) — UML 类图设计 v2
- **Shared infrastructure spec:** [docs/uml-design/README.md](docs/uml-design/README.md) — 共享层接口签名和集成规则
