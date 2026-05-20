# Purchase Management System (SRM) Design

Date: 2026-05-20

## Overview

采购管理系统（第三章），覆盖供应商管理、采购报价、采购订单、对账付款闭环四个子模块。零星采购复用采购订单表，通过 `order_type` 字段区分。

核心业务流：**供应商报价登记 → 手动创建采购订单（快照价格）→ 仓库收货入库 → 月度对账单 → 发票登记 → 付款申请**

## Scope

**包含：**
- 供应商主档案（主表 + 联系人子表 + 银行账户子表）
- 供应商价格登记簿（覆盖式报价 + 有效期）
- 采购订单 CRUD（含零星采购，下单时快照价格）
- 采购订单状态流转
- 月对账单（按供应商按月汇总已收货未对账的采购明细）
- 发票登记
- 付款申请

**不包含：**
- 物理收货入库（仓库模块处理，`ref_order_type = "purchase_order"`）
- MRP 驱动的采购需求自动生成
- 供应商协同门户
- 供应商绩效自动评分
- 采购框架协议管理
- 审批流（不接 workflow engine，状态由操作员手动推进）

## Proto Organization

三个 proto 文件，按业务域划分：

| 文件 | 服务 | 职责 |
|------|------|------|
| `supplier.proto` | `SupplierService` | 供应商主档案 CRUD |
| `purchase.proto` | `PurchaseService` | 供应商报价 + 采购订单 |
| `purchase_settlement.proto` | `PurchaseSettlementService` | 月对账 + 发票 + 付款 |

## Data Model

### suppliers

```sql
CREATE TABLE suppliers (
    supplier_id    BIGSERIAL PRIMARY KEY,
    supplier_code  VARCHAR(50) NOT NULL UNIQUE,
    supplier_name  VARCHAR(200) NOT NULL,
    short_name     VARCHAR(100),
    classification VARCHAR(10) NOT NULL DEFAULT 'C',
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_suppliers_status ON suppliers(status) WHERE deleted_at IS NULL;
```

`classification` 取值 A/B/C。`status`：1=待审核，2=合格，3=停用。

### supplier_contacts

```sql
CREATE TABLE supplier_contacts (
    contact_id     BIGSERIAL PRIMARY KEY,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id) ON DELETE CASCADE,
    contact_name   VARCHAR(100) NOT NULL,
    phone          VARCHAR(50),
    email          VARCHAR(100),
    position       VARCHAR(100),
    is_primary     BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supplier_contacts_supplier ON supplier_contacts(supplier_id);
```

主联系人 `is_primary = true`，每个供应商最多一个主联系人。

### supplier_bank_accounts

```sql
CREATE TABLE supplier_bank_accounts (
    bank_account_id BIGSERIAL PRIMARY KEY,
    supplier_id     BIGINT NOT NULL REFERENCES suppliers(supplier_id) ON DELETE CASCADE,
    bank_name       VARCHAR(200) NOT NULL,
    account_name    VARCHAR(200) NOT NULL,
    account_no      VARCHAR(100) NOT NULL,
    is_default      BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supplier_bank_accounts_supplier ON supplier_bank_accounts(supplier_id);
```

默认账户 `is_default = true`，每个供应商最多一个默认账户。

### supplier_prices

```sql
CREATE TABLE supplier_prices (
    price_id       BIGSERIAL PRIMARY KEY,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id),
    product_id     BIGINT NOT NULL,
    unit_price     DECIMAL(14,6) NOT NULL,
    valid_from     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_until    TIMESTAMPTZ NOT NULL,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supplier_prices_lookup
    ON supplier_prices(supplier_id, product_id, valid_until);
```

每次报价新增一行。`valid_until` 到期后该报价不再可用。下单时将价格快照到 `purchase_order_items.unit_price`，结算始终按快照价格。

查询当前有效报价：`WHERE NOW() BETWEEN valid_from AND valid_until`。

### purchase_orders

```sql
CREATE TABLE purchase_orders (
    po_id          BIGSERIAL PRIMARY KEY,
    po_no          VARCHAR(32) NOT NULL UNIQUE,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id),
    order_type     SMALLINT NOT NULL DEFAULT 1,
    status         SMALLINT NOT NULL DEFAULT 1,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_purchase_orders_supplier ON purchase_orders(supplier_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_purchase_orders_status ON purchase_orders(status) WHERE deleted_at IS NULL;
```

`order_type`：1=生产采购，2=零星采购。`po_no` 由文档编号服务生成（类型 `PO`）。

采购订单状态枚举：

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | PURCHASE_ORDER_STATUS_DRAFT | 草稿 |
| 2 | PURCHASE_ORDER_STATUS_SUBMITTED | 已提交 |
| 3 | PURCHASE_ORDER_STATUS_APPROVED | 已审批 |
| 4 | PURCHASE_ORDER_STATUS_PARTIAL_RECEIVED | 部分收货 |
| 5 | PURCHASE_ORDER_STATUS_FULLY_RECEIVED | 全部收货 |
| 6 | PURCHASE_ORDER_STATUS_RECONCILED | 已对账 |
| 7 | PURCHASE_ORDER_STATUS_CLOSED | 已关闭 |

### purchase_order_items

```sql
CREATE TABLE purchase_order_items (
    item_id        BIGSERIAL PRIMARY KEY,
    po_id          BIGINT NOT NULL REFERENCES purchase_orders(po_id) ON DELETE CASCADE,
    product_id     BIGINT NOT NULL,
    product_code   VARCHAR(100),
    product_name   VARCHAR(200),
    unit           VARCHAR(20),
    unit_price     DECIMAL(14,6) NOT NULL,
    quantity       DECIMAL(14,6) NOT NULL,
    received_qty   DECIMAL(14,6) NOT NULL DEFAULT 0,
    subtotal       DECIMAL(14,2) NOT NULL,
    remark         TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_purchase_order_items_po ON purchase_order_items(po_id);
```

`unit_price` 为下单时从 `supplier_prices` 快照的价格。`product_code`/`product_name`/`unit` 冗余存储，避免产品改名影响历史订单。

`received_qty` 由仓库入库时回写（`ref_order_type = "purchase_order"` 关联），采购模块通过查询库存入库记录汇总。

### purchase_statements

```sql
CREATE TABLE purchase_statements (
    statement_id   BIGSERIAL PRIMARY KEY,
    statement_no   VARCHAR(32) NOT NULL UNIQUE,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id),
    period_start   DATE NOT NULL,
    period_end     DATE NOT NULL,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

`statement_no` 由文档编号服务生成（类型 `PS`）。

对账单状态：1=待确认，2=已确认，3=有异议。

### purchase_statement_items

```sql
CREATE TABLE purchase_statement_items (
    item_id          BIGSERIAL PRIMARY KEY,
    statement_id     BIGINT NOT NULL REFERENCES purchase_statements(statement_id) ON DELETE CASCADE,
    po_id            BIGINT NOT NULL,
    po_no            VARCHAR(32),
    product_id       BIGINT NOT NULL,
    product_name     VARCHAR(200),
    quantity         DECIMAL(14,6) NOT NULL,
    unit_price       DECIMAL(14,6) NOT NULL,
    amount           DECIMAL(14,2) NOT NULL
);

CREATE INDEX idx_statement_items_statement ON purchase_statement_items(statement_id);
```

对账单明细由系统自动生成。生成逻辑：查找该供应商在 `period_start` ~ `period_end` 期间内状态为 `FULLY_RECEIVED` 或 `PARTIAL_RECEIVED` 且尚未被对账单关联的采购订单行项目，汇总到对账单明细中。

### purchase_invoices

```sql
CREATE TABLE purchase_invoices (
    invoice_id     BIGSERIAL PRIMARY KEY,
    invoice_no     VARCHAR(100) NOT NULL,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id),
    statement_id   BIGINT REFERENCES purchase_statements(statement_id),
    invoice_amount DECIMAL(14,2) NOT NULL,
    invoice_date   DATE NOT NULL,
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

发票状态：1=已登记，2=已核验。`invoice_no` 为供应商提供的发票号码（非系统生成）。

### purchase_payments

```sql
CREATE TABLE purchase_payments (
    payment_id     BIGSERIAL PRIMARY KEY,
    payment_no     VARCHAR(32) NOT NULL UNIQUE,
    supplier_id    BIGINT NOT NULL REFERENCES suppliers(supplier_id),
    invoice_id     BIGINT REFERENCES purchase_invoices(invoice_id),
    payment_amount DECIMAL(14,2) NOT NULL,
    payment_method VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,
    remark         TEXT,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

`payment_no` 由文档编号服务生成（类型 `PP`）。

付款状态：1=待审批，2=已审批，3=已付款。

## Document Sequence Entries

在 `document_sequences` 表中新增：

```sql
INSERT INTO document_sequences (doc_type, prefix, current_value, reset_rule) VALUES
('PO', 'PO-', 0, 'monthly'),
('PS', 'PS-', 0, 'monthly'),
('PP', 'PP-', 0, 'monthly');
```

编号格式：`PO-2026-05-00001`。

## Proto Definition

### supplier.proto

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum SupplierClassification {
  SUPPLIER_CLASSIFICATION_UNSPECIFIED = 0;
  SUPPLIER_CLASSIFICATION_A = 1;
  SUPPLIER_CLASSIFICATION_B = 2;
  SUPPLIER_CLASSIFICATION_C = 3;
}

enum SupplierStatus {
  SUPPLIER_STATUS_UNSPECIFIED = 0;
  SUPPLIER_STATUS_PENDING = 1;
  SUPPLIER_STATUS_QUALIFIED = 2;
  SUPPLIER_STATUS_DISABLED = 3;
}

message SupplierContact {
  int64 contact_id = 1;
  int64 supplier_id = 2;
  string contact_name = 3;
  string phone = 4;
  string email = 5;
  string position = 6;
  bool is_primary = 7;
}

message SupplierBankAccount {
  int64 bank_account_id = 1;
  int64 supplier_id = 2;
  string bank_name = 3;
  string account_name = 4;
  string account_no = 5;
  bool is_default = 6;
}

message Supplier {
  int64 supplier_id = 1;
  string supplier_code = 2;
  string supplier_name = 3;
  string short_name = 4;
  SupplierClassification classification = 5;
  SupplierStatus status = 6;
  string remark = 7;
  int64 operator_id = 8;
  int64 created_at = 9;
  int64 updated_at = 10;
  repeated SupplierContact contacts = 11;
  repeated SupplierBankAccount bank_accounts = 12;
}

message CreateSupplierRequest {
  string supplier_code = 1;
  string supplier_name = 2;
  string short_name = 3;
  SupplierClassification classification = 4;
  string remark = 5;
  repeated SupplierContactInput contacts = 6;
  repeated SupplierBankAccountInput bank_accounts = 7;
}

message SupplierContactInput {
  string contact_name = 1;
  string phone = 2;
  string email = 3;
  string position = 4;
  bool is_primary = 5;
}

message SupplierBankAccountInput {
  string bank_name = 1;
  string account_name = 2;
  string account_no = 3;
  bool is_default = 4;
}

message UpdateSupplierRequest {
  int64 supplier_id = 1;
  string supplier_name = 2;
  string short_name = 3;
  SupplierClassification classification = 4;
  string remark = 5;
  repeated SupplierContactInput contacts = 6;
  repeated SupplierBankAccountInput bank_accounts = 7;
}

message UpdateSupplierStatusRequest {
  int64 supplier_id = 1;
  SupplierStatus status = 2;
}

message ListSuppliersRequest {
  optional string keyword = 1;
  optional SupplierClassification classification = 2;
  optional SupplierStatus status = 3;
  optional PaginationParams pagination = 4;
}

message GetSupplierRequest {
  int64 supplier_id = 1;
}

message SupplierResponse {
  Supplier supplier = 1;
}

message SupplierListResponse {
  repeated Supplier items = 1;
  PaginationInfo pagination = 2;
}

service SupplierService {
  rpc CreateSupplier(CreateSupplierRequest) returns (U64Response);
  rpc UpdateSupplier(UpdateSupplierRequest) returns (BoolResponse);
  rpc DeleteSupplier(DeleteRequest) returns (BoolResponse);
  rpc GetSupplier(GetSupplierRequest) returns (SupplierResponse);
  rpc ListSuppliers(ListSuppliersRequest) returns (SupplierListResponse);
  rpc UpdateSupplierStatus(UpdateSupplierStatusRequest) returns (BoolResponse);
}
```

### purchase.proto

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum PurchaseOrderType {
  PURCHASE_ORDER_TYPE_UNSPECIFIED = 0;
  PURCHASE_ORDER_TYPE_PRODUCTION = 1;
  PURCHASE_ORDER_TYPE_MISCELLANEOUS = 2;
}

enum PurchaseOrderStatus {
  PURCHASE_ORDER_STATUS_UNSPECIFIED = 0;
  PURCHASE_ORDER_STATUS_DRAFT = 1;
  PURCHASE_ORDER_STATUS_SUBMITTED = 2;
  PURCHASE_ORDER_STATUS_APPROVED = 3;
  PURCHASE_ORDER_STATUS_PARTIAL_RECEIVED = 4;
  PURCHASE_ORDER_STATUS_FULLY_RECEIVED = 5;
  PURCHASE_ORDER_STATUS_RECONCILED = 6;
  PURCHASE_ORDER_STATUS_CLOSED = 7;
}

message SupplierPrice {
  int64 price_id = 1;
  int64 supplier_id = 2;
  int64 product_id = 3;
  string product_code = 4;
  string product_name = 5;
  string unit = 6;
  string unit_price = 7;
  int64 valid_from = 8;
  int64 valid_until = 9;
  int64 operator_id = 10;
  int64 created_at = 11;
}

message UpsertSupplierPriceRequest {
  int64 supplier_id = 1;
  int64 product_id = 2;
  string unit_price = 3;
  int64 valid_from = 4;
  int64 valid_until = 5;
}

message ListSupplierPricesRequest {
  optional int64 supplier_id = 1;
  optional int64 product_id = 2;
  optional bool active_only = 3;
  optional PaginationParams pagination = 4;
}

message SupplierPriceListResponse {
  repeated SupplierPrice items = 1;
  PaginationInfo pagination = 2;
}

message PurchaseOrderItem {
  int64 item_id = 1;
  int64 po_id = 2;
  int64 product_id = 3;
  string product_code = 4;
  string product_name = 5;
  string unit = 6;
  string unit_price = 7;
  string quantity = 8;
  string received_qty = 9;
  string subtotal = 10;
  string remark = 11;
}

message PurchaseOrder {
  int64 po_id = 1;
  string po_no = 2;
  int64 supplier_id = 3;
  string supplier_name = 4;
  PurchaseOrderType order_type = 5;
  PurchaseOrderStatus status = 6;
  string total_amount = 7;
  string remark = 8;
  int64 operator_id = 9;
  int64 created_at = 10;
  int64 updated_at = 11;
  repeated PurchaseOrderItem items = 12;
}

message CreatePurchaseOrderItem {
  int64 product_id = 1;
  string unit_price = 2;
  string quantity = 3;
  string remark = 4;
}

message CreatePurchaseOrderRequest {
  int64 supplier_id = 1;
  PurchaseOrderType order_type = 2;
  string remark = 3;
  repeated CreatePurchaseOrderItem items = 4;
}

message UpdatePurchaseOrderRequest {
  int64 po_id = 1;
  int64 supplier_id = 2;
  string remark = 3;
  repeated CreatePurchaseOrderItem items = 4;
}

message UpdatePurchaseOrderStatusRequest {
  int64 po_id = 1;
  PurchaseOrderStatus status = 2;
}

message ListPurchaseOrdersRequest {
  optional string keyword = 1;
  optional int64 supplier_id = 2;
  optional PurchaseOrderType order_type = 3;
  optional PurchaseOrderStatus status = 4;
  optional PaginationParams pagination = 5;
}

message GetPurchaseOrderRequest {
  int64 po_id = 1;
}

message PurchaseOrderResponse {
  PurchaseOrder purchase_order = 1;
}

message PurchaseOrderListResponse {
  repeated PurchaseOrder items = 1;
  PaginationInfo pagination = 2;
}

service PurchaseService {
  rpc UpsertSupplierPrice(UpsertSupplierPriceRequest) returns (U64Response);
  rpc ListSupplierPrices(ListSupplierPricesRequest) returns (SupplierPriceListResponse);
  rpc CreatePurchaseOrder(CreatePurchaseOrderRequest) returns (U64Response);
  rpc UpdatePurchaseOrder(UpdatePurchaseOrderRequest) returns (BoolResponse);
  rpc DeletePurchaseOrder(DeleteRequest) returns (BoolResponse);
  rpc GetPurchaseOrder(GetPurchaseOrderRequest) returns (PurchaseOrderResponse);
  rpc ListPurchaseOrders(ListPurchaseOrdersRequest) returns (PurchaseOrderListResponse);
  rpc UpdatePurchaseOrderStatus(UpdatePurchaseOrderStatusRequest) returns (BoolResponse);
}
```

### purchase_settlement.proto

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum StatementStatus {
  STATEMENT_STATUS_UNSPECIFIED = 0;
  STATEMENT_STATUS_PENDING = 1;
  STATEMENT_STATUS_CONFIRMED = 2;
  STATEMENT_STATUS_DISPUTED = 3;
}

enum InvoiceStatus {
  INVOICE_STATUS_UNSPECIFIED = 0;
  INVOICE_STATUS_REGISTERED = 1;
  INVOICE_STATUS_VERIFIED = 2;
}

enum PaymentStatus {
  PAYMENT_STATUS_UNSPECIFIED = 0;
  PAYMENT_STATUS_PENDING = 1;
  PAYMENT_STATUS_APPROVED = 2;
  PAYMENT_STATUS_PAID = 3;
}

message StatementItem {
  int64 item_id = 1;
  int64 statement_id = 2;
  int64 po_id = 3;
  string po_no = 4;
  int64 product_id = 5;
  string product_name = 6;
  string quantity = 7;
  string unit_price = 8;
  string amount = 9;
}

message PurchaseStatement {
  int64 statement_id = 1;
  string statement_no = 2;
  int64 supplier_id = 3;
  string supplier_name = 4;
  int64 period_start = 5;
  int64 period_end = 6;
  string total_amount = 7;
  StatementStatus status = 8;
  string remark = 9;
  int64 operator_id = 10;
  int64 created_at = 11;
  int64 updated_at = 12;
  repeated StatementItem items = 13;
}

message GenerateStatementRequest {
  int64 supplier_id = 1;
  int64 period_start = 2;
  int64 period_end = 3;
}

message UpdateStatementStatusRequest {
  int64 statement_id = 1;
  StatementStatus status = 2;
}

message ListStatementsRequest {
  optional int64 supplier_id = 1;
  optional StatementStatus status = 2;
  optional int64 period_start = 3;
  optional int64 period_end = 4;
  optional PaginationParams pagination = 5;
}

message GetStatementRequest {
  int64 statement_id = 1;
}

message StatementResponse {
  PurchaseStatement statement = 1;
}

message StatementListResponse {
  repeated PurchaseStatement items = 1;
  PaginationInfo pagination = 2;
}

message PurchaseInvoice {
  int64 invoice_id = 1;
  string invoice_no = 2;
  int64 supplier_id = 3;
  string supplier_name = 4;
  int64 statement_id = 5;
  string statement_no = 6;
  string invoice_amount = 7;
  int64 invoice_date = 8;
  InvoiceStatus status = 9;
  string remark = 10;
  int64 operator_id = 11;
  int64 created_at = 12;
}

message CreateInvoiceRequest {
  string invoice_no = 1;
  int64 supplier_id = 2;
  int64 statement_id = 3;
  string invoice_amount = 4;
  int64 invoice_date = 5;
  string remark = 6;
}

message UpdateInvoiceStatusRequest {
  int64 invoice_id = 1;
  InvoiceStatus status = 2;
}

message ListInvoicesRequest {
  optional int64 supplier_id = 1;
  optional int64 statement_id = 2;
  optional InvoiceStatus status = 3;
  optional PaginationParams pagination = 4;
}

message InvoiceListResponse {
  repeated PurchaseInvoice items = 1;
  PaginationInfo pagination = 2;
}

message PurchasePayment {
  int64 payment_id = 1;
  string payment_no = 2;
  int64 supplier_id = 3;
  string supplier_name = 4;
  int64 invoice_id = 5;
  string invoice_no = 6;
  string payment_amount = 7;
  string payment_method = 8;
  PaymentStatus status = 9;
  string remark = 10;
  int64 operator_id = 11;
  int64 created_at = 12;
  int64 updated_at = 13;
}

message CreatePaymentRequest {
  int64 supplier_id = 1;
  int64 invoice_id = 2;
  string payment_amount = 3;
  string payment_method = 4;
  string remark = 5;
}

message UpdatePaymentStatusRequest {
  int64 payment_id = 1;
  PaymentStatus status = 2;
}

message ListPaymentsRequest {
  optional int64 supplier_id = 1;
  optional PaymentStatus status = 2;
  optional PaginationParams pagination = 3;
}

message GetPaymentRequest {
  int64 payment_id = 1;
}

message PaymentResponse {
  PurchasePayment payment = 1;
}

message PaymentListResponse {
  repeated PurchasePayment items = 1;
  PaginationInfo pagination = 2;
}

service PurchaseSettlementService {
  rpc GenerateStatement(GenerateStatementRequest) returns (U64Response);
  rpc GetStatement(GetStatementRequest) returns (StatementResponse);
  rpc ListStatements(ListStatementsRequest) returns (StatementListResponse);
  rpc UpdateStatementStatus(UpdateStatementStatusRequest) returns (BoolResponse);
  rpc CreateInvoice(CreateInvoiceRequest) returns (U64Response);
  rpc ListInvoices(ListInvoicesRequest) returns (InvoiceListResponse);
  rpc UpdateInvoiceStatus(UpdateInvoiceStatusRequest) returns (BoolResponse);
  rpc CreatePayment(CreatePaymentRequest) returns (U64Response);
  rpc GetPayment(GetPaymentRequest) returns (PaymentResponse);
  rpc ListPayments(ListPaymentsRequest) returns (PaymentListResponse);
  rpc UpdatePaymentStatus(UpdatePaymentStatusRequest) returns (BoolResponse);
}
```

## Rust Models

### supplier.rs

```rust
#[derive(Debug, Serialize, Deserialize, Clone, Default, FromRow)]
pub struct Supplier {
    pub supplier_id: i64,
    pub supplier_code: String,
    pub supplier_name: String,
    pub short_name: Option<String>,
    pub classification: String,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierContact {
    pub contact_id: i64,
    pub supplier_id: i64,
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierBankAccount {
    pub bank_account_id: i64,
    pub supplier_id: i64,
    pub bank_name: String,
    pub account_name: String,
    pub account_no: String,
    pub is_default: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SupplierQuery {
    pub keyword: Option<String>,
    pub classification: Option<String>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

### supplier_price.rs

```rust
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierPrice {
    pub price_id: i64,
    pub supplier_id: i64,
    pub product_id: i64,
    pub unit_price: Decimal,
    pub valid_from: NaiveDateTime,
    pub valid_until: NaiveDateTime,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SupplierPriceQuery {
    pub supplier_id: Option<i64>,
    pub product_id: Option<i64>,
    pub active_only: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

### purchase_order.rs

```rust
#[derive(Debug, Serialize, Deserialize, Clone, Default, FromRow)]
pub struct PurchaseOrder {
    pub po_id: i64,
    pub po_no: String,
    pub supplier_id: i64,
    pub order_type: i16,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseOrderItem {
    pub item_id: i64,
    pub po_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub received_qty: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PurchaseOrderQuery {
    pub keyword: Option<String>,
    pub supplier_id: Option<i64>,
    pub order_type: Option<i16>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

### purchase_settlement.rs

```rust
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseStatement {
    pub statement_id: i64,
    pub statement_no: String,
    pub supplier_id: i64,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_amount: Decimal,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct StatementItem {
    pub item_id: i64,
    pub statement_id: i64,
    pub po_id: i64,
    pub po_no: Option<String>,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseInvoice {
    pub invoice_id: i64,
    pub invoice_no: String,
    pub supplier_id: i64,
    pub statement_id: Option<i64>,
    pub invoice_amount: Decimal,
    pub invoice_date: NaiveDate,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchasePayment {
    pub payment_id: i64,
    pub payment_no: String,
    pub supplier_id: i64,
    pub invoice_id: Option<i64>,
    pub payment_amount: Decimal,
    pub payment_method: Option<String>,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StatementQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<i16>,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InvoiceQuery {
    pub supplier_id: Option<i64>,
    pub statement_id: Option<i64>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PaymentQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

## Repository Layer

### SupplierRepo

```rust
impl SupplierRepo {
    pub async fn insert(executor: Executor<'_>, supplier: &Supplier) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, supplier: &Supplier) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, supplier_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, supplier_id: i64) -> Result<Option<Supplier>>;
    pub async fn query(pool: &PgPool, query: &SupplierQuery) -> Result<Vec<Supplier>>;
    pub async fn query_count(pool: &PgPool, query: &SupplierQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, supplier_id: i64, status: i16) -> Result<()>;
}

impl SupplierContactRepo {
    pub async fn insert_batch(executor: Executor<'_>, contacts: &[SupplierContact]) -> Result<()>;
    pub async fn delete_by_supplier(executor: Executor<'_>, supplier_id: i64) -> Result<()>;
    pub async fn find_by_supplier(pool: &PgPool, supplier_id: i64) -> Result<Vec<SupplierContact>>;
}

impl SupplierBankAccountRepo {
    pub async fn insert_batch(executor: Executor<'_>, accounts: &[SupplierBankAccount]) -> Result<()>;
    pub async fn delete_by_supplier(executor: Executor<'_>, supplier_id: i64) -> Result<()>;
    pub async fn find_by_supplier(pool: &PgPool, supplier_id: i64) -> Result<Vec<SupplierBankAccount>>;
}
```

### SupplierPriceRepo

```rust
impl SupplierPriceRepo {
    pub async fn insert(executor: Executor<'_>, price: &SupplierPrice) -> Result<i64>;
    pub async fn query(pool: &PgPool, query: &SupplierPriceQuery) -> Result<Vec<SupplierPrice>>;
    pub async fn query_count(pool: &PgPool, query: &SupplierPriceQuery) -> Result<i64>;
    pub async fn find_active(pool: &PgPool, supplier_id: i64, product_id: i64) -> Result<Option<SupplierPrice>>;
}
```

### PurchaseOrderRepo

```rust
impl PurchaseOrderRepo {
    pub async fn insert(executor: Executor<'_>, po: &PurchaseOrder) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, po: &PurchaseOrder) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, po_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, po_id: i64) -> Result<Option<PurchaseOrder>>;
    pub async fn query(pool: &PgPool, query: &PurchaseOrderQuery) -> Result<Vec<PurchaseOrder>>;
    pub async fn query_count(pool: &PgPool, query: &PurchaseOrderQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, po_id: i64, status: i16) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[PurchaseOrderItem]) -> Result<()>;
    pub async fn delete_by_po(executor: Executor<'_>, po_id: i64) -> Result<()>;
    pub async fn find_items_by_po(pool: &PgPool, po_id: i64) -> Result<Vec<PurchaseOrderItem>>;
}
```

### PurchaseSettlementRepo

```rust
impl StatementRepo {
    pub async fn insert(executor: Executor<'_>, statement: &PurchaseStatement) -> Result<i64>;
    pub async fn find_by_id(pool: &PgPool, statement_id: i64) -> Result<Option<PurchaseStatement>>;
    pub async fn query(pool: &PgPool, query: &StatementQuery) -> Result<Vec<PurchaseStatement>>;
    pub async fn query_count(pool: &PgPool, query: &StatementQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, statement_id: i64, status: i16) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[StatementItem]) -> Result<()>;
    pub async fn find_items(pool: &PgPool, statement_id: i64) -> Result<Vec<StatementItem>>;
}

impl InvoiceRepo {
    pub async fn insert(executor: Executor<'_>, invoice: &PurchaseInvoice) -> Result<i64>;
    pub async fn find_by_id(pool: &PgPool, invoice_id: i64) -> Result<Option<PurchaseInvoice>>;
    pub async fn query(pool: &PgPool, query: &InvoiceQuery) -> Result<Vec<PurchaseInvoice>>;
    pub async fn query_count(pool: &PgPool, query: &InvoiceQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, invoice_id: i64, status: i16) -> Result<()>;
}

impl PaymentRepo {
    pub async fn insert(executor: Executor<'_>, payment: &PurchasePayment) -> Result<i64>;
    pub async fn find_by_id(pool: &PgPool, payment_id: i64) -> Result<Option<PurchasePayment>>;
    pub async fn query(pool: &PgPool, query: &PaymentQuery) -> Result<Vec<PurchasePayment>>;
    pub async fn query_count(pool: &PgPool, query: &PaymentQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, payment_id: i64, status: i16) -> Result<()>;
}
```

## Service Layer

### Service Traits

```rust
#[async_trait]
pub trait SupplierService {
    async fn create(&self, operator_id: Option<i64>, req: CreateSupplierRequest, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, req: UpdateSupplierRequest, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, supplier_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, supplier_id: i64) -> Result<Option<Supplier>>;
    async fn list(&self, query: SupplierQuery) -> Result<PaginatedResult<Supplier>>;
    async fn update_status(&self, supplier_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait SupplierPriceService {
    async fn upsert(&self, operator_id: Option<i64>, req: UpsertSupplierPriceRequest, executor: Executor<'_>) -> Result<i64>;
    async fn list(&self, query: SupplierPriceQuery) -> Result<PaginatedResult<SupplierPrice>>;
}

#[async_trait]
pub trait PurchaseOrderService {
    async fn create(&self, operator_id: Option<i64>, req: CreatePurchaseOrderRequest, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, req: UpdatePurchaseOrderRequest, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, po_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, po_id: i64) -> Result<Option<PurchaseOrder>>;
    async fn list(&self, query: PurchaseOrderQuery) -> Result<PaginatedResult<PurchaseOrder>>;
    async fn update_status(&self, po_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait StatementService {
    async fn generate(&self, operator_id: Option<i64>, req: GenerateStatementRequest, executor: Executor<'_>) -> Result<i64>;
    async fn get_by_id(&self, statement_id: i64) -> Result<Option<PurchaseStatement>>;
    async fn list(&self, query: StatementQuery) -> Result<PaginatedResult<PurchaseStatement>>;
    async fn update_status(&self, statement_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait InvoiceService {
    async fn create(&self, operator_id: Option<i64>, req: CreateInvoiceRequest, executor: Executor<'_>) -> Result<i64>;
    async fn list(&self, query: InvoiceQuery) -> Result<PaginatedResult<PurchaseInvoice>>;
    async fn update_status(&self, invoice_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait PaymentService {
    async fn create(&self, operator_id: Option<i64>, req: CreatePaymentRequest, executor: Executor<'_>) -> Result<i64>;
    async fn get_by_id(&self, payment_id: i64) -> Result<Option<PurchasePayment>>;
    async fn list(&self, query: PaymentQuery) -> Result<PaginatedResult<PurchasePayment>>;
    async fn update_status(&self, payment_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
```

### Business Logic

**SupplierService.create:**
1. 开启事务
2. 校验 `supplier_code` 唯一性
3. 插入主表
4. 批量插入联系人、银行账户
5. 提交事务，返回 `supplier_id`

**SupplierService.update:**
1. 查询现有供应商，校验存在且未删除
2. 更新主表（`supplier_code` 不可改）
3. 删除旧联系人/银行账户 → 批量插入新的（整体替换）

**SupplierPriceService.upsert:**
1. 插入新报价行（不删除旧报价，通过 `valid_until` 过期）

**PurchaseOrderService.create:**
1. 开启事务
2. 调用 `document_sequence.next_number(executor, "PO")` 生成编号
3. 校验行项目 `product_id` 存在性，冗余写入 `product_code`/`product_name`/`unit`
4. 计算 `subtotal = unit_price * quantity`，聚合 `total_amount`
5. 插入主表 + 批量插入行项目
6. 提交事务

**PurchaseOrderService.update:**
1. 查询现有订单，校验状态为 Draft
2. 重新计算 subtotal / total_amount
3. 更新主表，删除旧行项目 → 插入新行项目

**PurchaseOrderService.update_status:**
状态转换白名单：
- Draft → Submitted
- Submitted → Approved
- Approved → PartialReceived / FullyReceived（由仓库入库触发或手动）
- FullyReceived → Reconciled
- Reconciled → Closed
- 其他转换 → `ServiceError::BusinessValidation`

**StatementService.generate:**
1. 开启事务
2. 调用 `document_sequence.next_number(executor, "PS")` 生成编号
3. 查询该供应商在 `period_start` ~ `period_end` 期间内，状态为 FullyReceived 或 PartialReceived 且尚未被对账单关联的采购订单行项目
4. 汇总生成对账单明细，计算 `total_amount`
5. 将关联的采购订单状态更新为 Reconciled
6. 提交事务

**InvoiceService.create:**
1. 插入发票记录，关联对账单（可选）

**PaymentService.create:**
1. 调用 `document_sequence.next_number(executor, "PP")` 生成编号
2. 插入付款申请

## Handler Layer

每个 service 一个 handler 文件：
- `abt-grpc/src/handlers/supplier.rs`
- `abt-grpc/src/handlers/purchase.rs`
- `abt-grpc/src/handlers/purchase_settlement.rs`

遵循现有 handler 模式：Proto request → Service call → Proto response。所有 `anyhow::Error` 通过 `err_to_status` 转换为 `tonic::Status`。事务由 handler 层管理。

## Registration

- `abt-grpc/src/server.rs`: 注册 `SupplierServiceServer`、`PurchaseServiceServer`、`PurchaseSettlementServiceServer`
- `abt/src/lib.rs`: 添加 `get_supplier_service`、`get_supplier_price_service`、`get_purchase_order_service`、`get_statement_service`、`get_invoice_service`、`get_payment_service` 工厂函数

## Status Enum Mappings

### Supplier Status

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SUPPLIER_STATUS_PENDING | 待审核 |
| 2 | SUPPLIER_STATUS_QUALIFIED | 合格 |
| 3 | SUPPLIER_STATUS_DISABLED | 停用 |

### Supplier Classification

| DB (String) | Proto Enum | 含义 |
|-------------|-----------|------|
| A | SUPPLIER_CLASSIFICATION_A | A级 |
| B | SUPPLIER_CLASSIFICATION_B | B级 |
| C | SUPPLIER_CLASSIFICATION_C | C级 |

### Purchase Order Status

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | PURCHASE_ORDER_STATUS_DRAFT | 草稿 |
| 2 | PURCHASE_ORDER_STATUS_SUBMITTED | 已提交 |
| 3 | PURCHASE_ORDER_STATUS_APPROVED | 已审批 |
| 4 | PURCHASE_ORDER_STATUS_PARTIAL_RECEIVED | 部分收货 |
| 5 | PURCHASE_ORDER_STATUS_FULLY_RECEIVED | 全部收货 |
| 6 | PURCHASE_ORDER_STATUS_RECONCILED | 已对账 |
| 7 | PURCHASE_ORDER_STATUS_CLOSED | 已关闭 |

### Statement Status

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | STATEMENT_STATUS_PENDING | 待确认 |
| 2 | STATEMENT_STATUS_CONFIRMED | 已确认 |
| 3 | STATEMENT_STATUS_DISPUTED | 有异议 |

### Invoice Status

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | INVOICE_STATUS_REGISTERED | 已登记 |
| 2 | INVOICE_STATUS_VERIFIED | 已核验 |

### Payment Status

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | PAYMENT_STATUS_PENDING | 待审批 |
| 2 | PAYMENT_STATUS_APPROVED | 已审批 |
| 3 | PAYMENT_STATUS_PAID | 已付款 |

## File List

| Layer | New Files |
|-------|-----------|
| Proto | `proto/abt/v1/supplier.proto`, `proto/abt/v1/purchase.proto`, `proto/abt/v1/purchase_settlement.proto` |
| Model | `abt/src/models/supplier.rs`, `abt/src/models/supplier_price.rs`, `abt/src/models/purchase_order.rs`, `abt/src/models/purchase_settlement.rs` |
| Repository | `abt/src/repositories/supplier_repo.rs`, `abt/src/repositories/supplier_price_repo.rs`, `abt/src/repositories/purchase_order_repo.rs`, `abt/src/repositories/purchase_settlement_repo.rs` |
| Service | `abt/src/service/supplier_service.rs`, `abt/src/service/supplier_price_service.rs`, `abt/src/service/purchase_order_service.rs`, `abt/src/service/statement_service.rs`, `abt/src/service/invoice_service.rs`, `abt/src/service/payment_service.rs` |
| Impl | `abt/src/implt/supplier_service_impl.rs`, `abt/src/implt/supplier_price_service_impl.rs`, `abt/src/implt/purchase_order_service_impl.rs`, `abt/src/implt/statement_service_impl.rs`, `abt/src/implt/invoice_service_impl.rs`, `abt/src/implt/payment_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/supplier.rs`, `abt-grpc/src/handlers/purchase.rs`, `abt-grpc/src/handlers/purchase_settlement.rs` |
| Migration | `abt/migrations/045_create_supplier_tables.sql`, `abt/migrations/045_create_purchase_tables.sql` |
