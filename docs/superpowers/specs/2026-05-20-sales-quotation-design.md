# Sales Quotation Module Design

Date: 2026-05-20

## Overview

销售报价模块是销售管理系统（第二章）的第一个子模块。支持创建包含多行产品的报价单，管理报价生命周期（草稿 → 提交 → 接受/拒绝/过期）。

同时引入轻量级文档编号服务，为当前及后续所有需要单据编号的模块（报价单、订单、采购单、发货单等）提供统一编号生成能力。

## Scope

**包含：**
- 文档编号服务（`document_sequences` 表 + 通用序号生成）
- 报价单 CRUD（主表 + 行项目）
- 报价单状态流转（Draft → Submitted → Accepted/Rejected/Expired）

**不包含：**
- 客户主数据（客户名称为纯文本字段）
- BOM 成本自动计算（手动填写价格）
- 审批流（不接 workflow engine）
- 报价单转订单（后续订单模块再做）

## Data Model

### document_sequences

```sql
CREATE TABLE document_sequences (
    sequence_id   BIGSERIAL PRIMARY KEY,
    doc_type      VARCHAR(20) NOT NULL UNIQUE,
    prefix        VARCHAR(10) NOT NULL,
    current_value INTEGER NOT NULL DEFAULT 0,
    reset_rule    VARCHAR(20) NOT NULL DEFAULT 'monthly',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule)
VALUES ('QT', 'QT-', 0, 'monthly');
```

编号生成逻辑：`SELECT ... FOR UPDATE` 锁行 → `current_value + 1` → 生成 `QT-2026-05-00001` → `UPDATE`。按 `reset_rule` 月度/年度重置序号。

### quotations

```sql
CREATE TABLE quotations (
    quotation_id   BIGSERIAL PRIMARY KEY,
    quotation_no   VARCHAR(32) NOT NULL UNIQUE,
    customer_name  VARCHAR(200) NOT NULL,
    contact_person VARCHAR(100),
    contact_phone  VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    valid_until    TIMESTAMPTZ,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_quotations_status ON quotations(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_quotations_customer ON quotations(customer_name) WHERE deleted_at IS NULL;
```

### quotation_items

```sql
CREATE TABLE quotation_items (
    item_id       BIGSERIAL PRIMARY KEY,
    quotation_id  BIGINT NOT NULL REFERENCES quotations(quotation_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    unit_price    DECIMAL(14,6) NOT NULL,
    quantity      DECIMAL(14,6) NOT NULL,
    discount      DECIMAL(5,4) NOT NULL DEFAULT 1.0,
    subtotal      DECIMAL(14,2) NOT NULL,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_quotation_items_quotation ON quotation_items(quotation_id);
```

`unit_price`/`quantity` 用 `Decimal(14,6)` 与系统约定一致。`subtotal`/`total_amount` 用 `Decimal(14,2)` 因为是金额。行项目无独立软删除，随主表级联。

## Proto Definition

File: `proto/abt/v1/quotation.proto`

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum QuotationStatus {
  QUOTATION_STATUS_UNSPECIFIED = 0;
  QUOTATION_STATUS_DRAFT = 1;
  QUOTATION_STATUS_SUBMITTED = 2;
  QUOTATION_STATUS_ACCEPTED = 3;
  QUOTATION_STATUS_REJECTED = 4;
  QUOTATION_STATUS_EXPIRED = 5;
}

message QuotationItem {
  int64 item_id = 1;
  int64 quotation_id = 2;
  int64 product_id = 3;
  string product_code = 4;
  string product_name = 5;
  string unit = 6;
  double unit_price = 7;
  double quantity = 8;
  double discount = 9;
  double subtotal = 10;
  string remark = 11;
}

message Quotation {
  int64 quotation_id = 1;
  string quotation_no = 2;
  string customer_name = 3;
  string contact_person = 4;
  string contact_phone = 5;
  QuotationStatus status = 6;
  double total_amount = 7;
  string remark = 8;
  int64 valid_until = 9;
  int64 created_at = 10;
  int64 updated_at = 11;
  int64 operator_id = 12;
  repeated QuotationItem items = 13;
}

message CreateQuotationRequest {
  string customer_name = 1;
  string contact_person = 2;
  string contact_phone = 3;
  string remark = 4;
  int64 valid_until = 5;
  repeated CreateQuotationItem items = 6;
}

message CreateQuotationItem {
  int64 product_id = 1;
  double unit_price = 2;
  double quantity = 3;
  double discount = 4;
  string remark = 5;
}

message UpdateQuotationRequest {
  int64 quotation_id = 1;
  string customer_name = 2;
  string contact_person = 3;
  string contact_phone = 4;
  string remark = 5;
  int64 valid_until = 6;
  repeated CreateQuotationItem items = 7;
}

message ListQuotationsRequest {
  optional string keyword = 1;
  optional QuotationStatus status = 2;
  optional PaginationParams pagination = 3;
}

message GetQuotationRequest {
  int64 quotation_id = 1;
}

message UpdateQuotationStatusRequest {
  int64 quotation_id = 1;
  QuotationStatus status = 2;
}

message QuotationResponse {
  Quotation quotation = 1;
}

message QuotationListResponse {
  repeated Quotation items = 1;
  PaginationInfo pagination = 2;
}

service QuotationService {
  rpc CreateQuotation(CreateQuotationRequest) returns (U64Response);
  rpc UpdateQuotation(UpdateQuotationRequest) returns (BoolResponse);
  rpc DeleteQuotation(DeleteQuotationRequest) returns (BoolResponse);
  rpc GetQuotation(GetQuotationRequest) returns (QuotationResponse);
  rpc ListQuotations(ListQuotationsRequest) returns (QuotationListResponse);
  rpc UpdateQuotationStatus(UpdateQuotationStatusRequest) returns (BoolResponse);
}
```

Key decisions:
- `quotation_no` 系统自动生成，调用文档编号服务
- 行项目 `product_code`/`product_name`/`unit` 冗余存储，避免产品改名影响历史报价
- `UpdateQuotation` 整体替换行项目（先删后插），不做行级增删改
- `DeleteQuotation` 仅允许 Draft 状态

## Rust Models

File: `abt/src/models/quotation.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Quotation {
    pub quotation_id: i64,
    pub quotation_no: String,
    pub customer_name: String,
    pub contact_person: Option<String>,
    pub contact_phone: Option<String>,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub valid_until: Option<NaiveDateTime>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
    pub items: Vec<QuotationItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuotationItem {
    pub item_id: i64,
    pub quotation_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct QuotationQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

File: `abt/src/models/document_sequence.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentSequence {
    pub sequence_id: i64,
    pub doc_type: String,
    pub prefix: String,
    pub current_value: i32,
    pub reset_rule: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
```

## Repository Layer

### DocumentSequenceRepo

```rust
impl DocumentSequenceRepo {
    pub async fn next_number(executor: Executor<'_>, doc_type: &str) -> Result<String>;
    pub async fn ensure_sequence(executor: Executor<'_>, doc_type: &str, prefix: &str, reset_rule: &str) -> Result<()>;
}
```

### QuotationRepo

```rust
impl QuotationRepo {
    pub async fn insert(executor: Executor<'_>, quotation: &Quotation) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, quotation: &Quotation) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, quotation_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, quotation_id: i64) -> Result<Option<Quotation>>;
    pub async fn query(pool: &PgPool, query: &QuotationQuery) -> Result<Vec<Quotation>>;
    pub async fn query_count(pool: &PgPool, query: &QuotationQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, quotation_id: i64, status: i16) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[QuotationItem]) -> Result<()>;
    pub async fn delete_by_quotation(executor: Executor<'_>, quotation_id: i64) -> Result<()>;
    pub async fn find_by_quotation_id(pool: &PgPool, quotation_id: i64) -> Result<Vec<QuotationItem>>;
}
```

## Service Layer

### Traits

```rust
#[async_trait]
pub trait QuotationService {
    async fn create(&self, operator_id: Option<i64>, req: CreateQuotationRequest, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, req: UpdateQuotationRequest, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, quotation_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, quotation_id: i64) -> Result<Option<Quotation>>;
    async fn list(&self, query: QuotationQuery) -> Result<PaginatedResult<Quotation>>;
    async fn update_status(&self, quotation_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait DocumentSequenceService {
    async fn next_number(&self, executor: Executor<'_>, doc_type: &str) -> Result<String>;
}
```

### Business Logic

**create:**
1. 开启事务
2. 调用 `document_sequence.next_number(executor, "QT")` 生成编号
3. 校验行项目 `product_id` 存在性，冗余写入 `product_code`/`product_name`/`unit`
4. 计算每个 item 的 `subtotal = unit_price * quantity * discount`
5. 聚合 `total_amount = sum(subtotals)`
6. 插入主表 + 批量插入行项目
7. 提交事务，返回 `quotation_id`

**update:**
1. 查询现有报价单，校验状态为 Draft（非 Draft 不可编辑）
2. 重新计算 subtotal / total_amount
3. 更新主表，删除旧行项目 → 插入新行项目

**delete:**
1. 校验状态为 Draft → 软删除主表

**update_status:**
状态转换白名单：
- Draft → Submitted
- Submitted → Accepted / Rejected
- Draft → Expired
- 其他转换 → `ServiceError::BusinessValidation`

**get_by_id / list:**
查询主表后，二次查询填充 items。

## Handler Layer

File: `abt-grpc/src/handlers/quotation.rs`

遵循现有 handler 模式：Proto request → Service call → Proto response。所有 `anyhow::Error` 通过 `err_to_status` 转换为 `tonic::Status`。

事务由 handler 层管理（`pool().begin()` → 传 executor 给 service → `tx.commit()`）。

Model → Proto 转换函数：`quotation_to_proto`、`quotation_item_to_proto`、`status_i16_to_proto`。

## Registration

- `abt-grpc/src/server.rs`: 注册 `QuotationServiceServer`
- `abt/src/lib.rs`: 添加 `get_quotation_service` 和 `get_document_sequence_service` 工厂函数

## Status Enum Mapping

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | QUOTATION_STATUS_DRAFT | 草稿 |
| 2 | QUOTATION_STATUS_SUBMITTED | 已提交 |
| 3 | QUOTATION_STATUS_ACCEPTED | 已接受 |
| 4 | QUOTATION_STATUS_REJECTED | 已拒绝 |
| 5 | QUOTATION_STATUS_EXPIRED | 已过期 |

## File List

| Layer | New Files |
|-------|-----------|
| Proto | `proto/abt/v1/quotation.proto` |
| Model | `abt/src/models/quotation.rs`, `abt/src/models/document_sequence.rs` |
| Repository | `abt/src/repositories/quotation_repo.rs`, `abt/src/repositories/document_sequence_repo.rs` |
| Service | `abt/src/service/quotation_service.rs`, `abt/src/service/document_sequence_service.rs` |
| Impl | `abt/src/implt/quotation_service_impl.rs`, `abt/src/implt/document_sequence_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/quotation.rs` |
| Migration | `abt/migrations/XXX_create_quotations.sql`, `abt/migrations/XXX_create_document_sequences.sql` |
