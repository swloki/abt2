---
title: "feat: Add Sales Quotation Module"
type: feat
status: active
date: 2026-05-20
origin: docs/superpowers/specs/2026-05-20-sales-quotation-design.md
---

# feat: Add Sales Quotation Module

## Summary

实现销售报价模块和轻量级文档编号服务。报价单支持多行项目、状态流转（Draft→Submitted→Accepted/Rejected/Expired），编号服务为后续所有需要单据编号的模块提供统一能力。遵循系统现有六层架构。

---

## Requirements

- R1. 创建/编辑/删除报价单（主表 + 多行项目），仅草稿可编辑和删除
- R2. 报价单状态流转：Draft→Submitted→Accepted/Rejected/Expired，非法转换拒绝
- R3. 报价单列表查询（模糊搜索 + 状态筛选 + 分页）
- R4. 报价单详情查询（含行项目）
- R5. 文档编号服务：通用序号生成，支持月度/年度重置，并发安全
- R6. 报价单编号自动生成，格式 `QTYYYY-MM-NNNNN`

---

## Scope Boundaries

- 不含客户主数据（客户名称为纯文本字段）
- 不含 BOM 成本自动计算（手动填写价格）
- 不含审批流（不接 workflow engine）
- 不含报价单转订单（后续订单模块再做）
- 不含前端代码

---

## Context & Research

### Relevant Code and Patterns

- Proto 模式：`proto/abt/v1/product.proto`（CRUD + 列表分页）、`proto/abt/v1/base.proto`（PaginationParams/PaginationInfo/通用响应）
- Model 模式：`abt/src/models/product.rs`（FromRow 实现、Query struct、JSONB meta）
- Repository 模式：`abt/src/repositories/product_repo.rs`（Executor 参数、build_fuzzy_pattern、分页查询）
- Service 模式：`abt/src/service/product_service.rs`（async_trait）、`abt/src/implt/product_service_impl.rs`（Arc<PgPool>）
- Handler 模式：`abt-grpc/src/handlers/product.rs`（err_to_status、事务管理）
- 错误处理：`common/src/error.rs`（ServiceError::NotFound/Conflict/BusinessValidation + err_to_status）
- 工厂函数：`abt/src/lib.rs`（`get_*_service` 模式）
- 服务注册：`abt-grpc/src/server.rs`（add_service with_interceptor）
- 分页工具：`abt/src/repositories/mod.rs`（PaginationParams、PaginatedResult、build_fuzzy_pattern）
- 编号参考：`abt/src/repositories/notification_repo.rs`（可能有序列号生成模式）

---

## Key Technical Decisions

- **文档编号用 SELECT FOR UPDATE 行锁**：保证并发安全，避免序号冲突。事务内使用，与报价创建同一事务
- **行项目先删后插**：更新时整体替换行项目，不做 diff。简化逻辑，避免行级状态管理
- **产品信息冗余存储**：行项目中存储 product_code/product_name/unit，避免产品改名影响历史报价
- **状态用 i16 存储**：与 Proto QuotationStatus 枚举在 handler 层互转，model 层保持简单
- **total_amount 主表存储**：创建/更新时从行项目聚合计算，查询时无需 JOIN 汇总

---

## Implementation Units

### U1. Proto Definition

**Goal:** 定义报价单相关的 messages、enum、service RPC

**Requirements:** R1, R2, R3, R4, R6

**Dependencies:** None

**Files:**
- Create: `proto/abt/v1/quotation.proto`

**Approach:**
- 遵循现有 proto 命名惯例（CreateXxxRequest、XxxResponse、XxxListResponse）
- 引用 base.proto 的 PaginationParams/PaginationInfo/U64Response/BoolResponse/DeleteXxxRequest
- QuotationStatus 枚举：UNSPECIFIED(0)、DRAFT(1)、SUBMITTED(2)、ACCEPTED(3)、REJECTED(4)、EXPIRED(5)
- 行项目 product_code/product_name/unit 冗余字段
- CreateQuotationItem 包含 product_id/unit_price/quantity/discount/remark
- UpdateQuotationRequest 整体替换行项目

**Patterns to follow:** `proto/abt/v1/product.proto`

**Test expectation:** `cargo build` 成功生成 proto 代码到 `abt-grpc/src/generated/`

**Verification:** `cargo build` 无错误，generated 文件包含 QuotationService

---

### U2. Database Migrations

**Goal:** 创建 quotations、quotation_items、document_sequences 三张表及索引

**Requirements:** R1, R5

**Dependencies:** U1

**Files:**
- Create: `abt/migrations/045_create_document_sequences.sql`
- Create: `abt/migrations/046_create_quotations.sql`

**Approach:**
- 两个独立 migration 文件，document_sequences 先于 quotations
- document_sequences 初始化 QT 序列记录
- quotations 遵循系统惯例（soft delete via deleted_at、operator_id 审计）
- quotation_items 的 unit_price/quantity 用 Decimal(14,6)，subtotal/total_amount 用 Decimal(14,2)

**Test expectation:** `cargo clippy` 通过，migration 文件 SQL 语法正确

**Verification:** `cargo build` 通过，表结构符合设计规范

---

### U3. Model: Document Sequence & Quotation

**Goal:** 实现文档编号和报价单的数据模型

**Requirements:** R1, R3, R4, R5, R6

**Dependencies:** U2

**Files:**
- Create: `abt/src/models/document_sequence.rs`
- Create: `abt/src/models/quotation.rs`
- Modify: `abt/src/models/mod.rs`

**Approach:**
- DocumentSequence struct：sequence_id, doc_type, prefix, current_value, reset_rule, created_at, updated_at
- Quotation struct + QuotationItem struct + QuotationQuery struct
- 手动 FromRow 实现

**Patterns to follow:** `abt/src/models/product.rs`

**Verification:** `cargo clippy` 通过

---

### U4. Repository: Document Sequence

**Goal:** 实现文档编号数据库访问层

**Requirements:** R5, R6

**Dependencies:** U3

**Files:**
- Create: `abt/src/repositories/document_sequence_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

**Approach:**
- next_number(executor, doc_type) → SELECT FOR UPDATE 锁行 → 检查 reset_rule 是否需要重置 → current_value + 1 → 生成格式化编号（prefix + YYYY-MM + 序号）→ UPDATE → 返回编号字符串
- ensure_sequence(executor, doc_type, prefix, reset_rule) → INSERT ON CONFLICT DO NOTHING

**Patterns to follow:** `abt/src/repositories/product_repo.rs`

**Test scenarios:**
- Happy path: 连续调用 next_number("QT") 生成 QT202605-00001、QT202605-00002
- Edge case: ensure_sequence 对已存在的 doc_type 不报错（幂等）
- Edge case: 月度重置逻辑（新月份序号从 1 开始）

**Verification:** `cargo clippy` 通过

---

### U5. Repository: Quotation

**Goal:** 实现报价单和行项目的数据访问层

**Requirements:** R1, R3, R4

**Dependencies:** U3

**Files:**
- Create: `abt/src/repositories/quotation_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

**Approach:**
- Quotation.items 默认空 Vec，find_by_id 时通过二次查询填充
- Repository 方法：insert, update, soft_delete, find_by_id, query, query_count, update_status, insert_items, delete_by_quotation, find_by_quotation_id
- query 支持 keyword（ILIKE quotation_no 或 customer_name）+ status 筛选 + 分页
- 使用 build_fuzzy_pattern 做模糊搜索

**Patterns to follow:** `abt/src/repositories/product_repo.rs`

**Test scenarios:**
- Happy path: insert + find_by_id 返回完整数据
- Happy path: query with keyword + status filter + pagination
- Edge case: find_by_id 查不存在的 ID 返回 None
- Edge case: soft_delete 后 find_by_id 返回 None

**Verification:** `cargo clippy` 通过

---

### U6. Service Trait: Document Sequence & Quotation

**Goal:** 定义文档编号和报价单的业务接口

**Requirements:** R1, R2, R5, R6

**Dependencies:** U3

**Files:**
- Create: `abt/src/service/document_sequence_service.rs`
- Create: `abt/src/service/quotation_service.rs`
- Modify: `abt/src/service/mod.rs`

**Approach:**
- DocumentSequenceService trait：next_number
- QuotationService trait：create, update, delete, get_by_id, list, update_status

**Patterns to follow:** `abt/src/service/product_service.rs`

**Verification:** `cargo clippy` 通过

---

### U7. Service Impl: Document Sequence

**Goal:** 实现文档编号的业务逻辑

**Requirements:** R5, R6

**Dependencies:** U4, U6

**Files:**
- Create: `abt/src/implt/document_sequence_service_impl.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`（添加工厂函数）

**Approach:**
- DocumentSequenceServiceImpl：包装 repo 的 next_number 和 ensure_sequence
- 持有 Arc<PgPool>（虽然 next_number 接收 executor，但 trait 一致性需要）

**Patterns to follow:** `abt/src/implt/product_service_impl.rs`

**Test scenarios:**
- Happy path: 连续调用生成递增编号
- Edge case: 跨月重置序号回到 1

**Verification:** `cargo clippy` 通过

---

### U8. Service Impl: Quotation

**Goal:** 实现报价单的业务逻辑

**Requirements:** R1, R2, R6

**Dependencies:** U5, U6

**Files:**
- Create: `abt/src/implt/quotation_service_impl.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`（添加工厂函数）

**Approach:**
- QuotationServiceImpl：
  - create：在同一 executor（事务）内调用 next_number → 校验 product_id 存在性 → 计算 subtotal/total_amount → insert 主表 → insert_items
  - update：查询现有报价 → 校验 Draft 状态 → 重新计算 → update 主表 → delete_by_quotation → insert_items
  - delete：校验 Draft 状态 → soft_delete
  - update_status：状态转换白名单校验（Draft→Submitted, Submitted→Accepted/Rejected, Draft→Expired）
  - get_by_id：查询主表 + 二次查询填充 items
  - list：query + query_count + PaginatedResult
- 状态转换失败抛 ServiceError::BusinessValidation
- 非 Draft 编辑/删除抛 ServiceError::BusinessValidation
- 产品不存在抛 ServiceError::NotFound

**Patterns to follow:** `abt/src/implt/product_service_impl.rs`、`abt/src/implt/user_service_impl.rs`（ServiceError 使用模式）

**Test scenarios:**
- Happy path: create 生成编号、计算金额、返回 ID
- Happy path: update 草稿报价单，行项目整体替换
- Happy path: update_status Draft→Submitted→Accepted
- Error path: update 非 Draft 报价单 → BusinessValidation
- Error path: update_status 非法转换（如 Accepted→Draft）→ BusinessValidation
- Error path: create 行项目 product_id 不存在 → NotFound
- Error path: delete 非 Draft 报价单 → BusinessValidation

**Verification:** `cargo clippy` 通过

---

### U9. gRPC Handler + Server Registration

**Goal:** 实现 Proto 层到 Service 层的转换，注册到 gRPC server

**Requirements:** R1, R2, R3, R4

**Dependencies:** U1, U7, U8

**Files:**
- Create: `abt-grpc/src/handlers/quotation.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/server.rs`

**Approach:**
- QuotationHandler 实现 QuotationService trait
- 每个 RPC：提取 request → AppState::get() → get_quotation_service → 调用 service → map_err(err_to_status) → 构造 response
- create/update 操作由 handler 管理 tx.commit()，service 内部通过 executor 操作
- Model→Proto 转换函数：quotation_to_proto、quotation_item_to_proto、status_i16_to_proto
- server.rs 中 add_service(QuotationServiceServer::with_interceptor(...))

**Patterns to follow:** `abt-grpc/src/handlers/bom_category.rs`（事务管理模式）、`abt-grpc/src/handlers/product.rs`（CRUD handler）

**Test scenarios:**
- Happy path: CreateQuotation 返回 ID
- Happy path: GetQuotation 返回含 items 的完整数据
- Happy path: ListQuotations 分页响应
- Error path: UpdateQuotation 非 Draft → gRPC FailedPrecondition
- Error path: UpdateQuotationStatus 非法转换 → gRPC FailedPrecondition

**Verification:** `cargo clippy` 通过，`cargo build` 通过

---

## System-Wide Impact

- **Interaction graph:** 报价服务引用 ProductRepo（校验 product_id 存在性）、DocumentSequenceRepo（生成编号）
- **Error propagation:** ServiceError::BusinessValidation / NotFound 通过 err_to_status 转为 gRPC Status
- **State lifecycle risks:** 无部分写入风险——create/update 在同一事务内完成编号生成 + 主表 + 行项目
- **API surface parity:** 新增独立 service，不影响现有 API
- **Unchanged invariants:** 现有 products/inventory/bom 模块不受影响

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 并发编号冲突 | SELECT FOR UPDATE 行锁 + 事务内操作 |
| 行项目更新一致性 | 先删后插在同一事务内 |
| 产品 ID 校验性能 | 批量查询 product_id 存在性（单次 SQL IN） |

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-20-sales-quotation-design.md](docs/superpowers/specs/2026-05-20-sales-quotation-design.md)
