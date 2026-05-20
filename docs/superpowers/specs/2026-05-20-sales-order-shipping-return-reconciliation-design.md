---
name: sales-order-shipping-return-reconciliation
description: 销售管理系统剩余四个子模块设计，包含销售订单、发货申请、销售退货、月对账单
---

# Sales Order, Shipping, Return & Reconciliation Design

Date: 2026-05-20

## Overview

销售管理系统第二章剩余四个子模块的统一设计。与已完成的销售报价模块（Quotation）构成完整的销售管理链路：

```
报价单 (已完成) → 销售订单 → 发货申请 → 销售退货
                                         ↓
                                    月对账单（汇总）
```

## Scope

**包含：**
- 销售订单（Sales Order）：创建/编辑/状态流转，支持从报价单转入
- 发货申请（Shipping Request）：分批发货，两步确认（销售确认 + 仓库出库）
- 销售退货（Sales Return）：必须关联原发货单，完整退货流程
- 月对账单（Reconciliation Statement）：按客户按月汇总发货 + 退货 + 手动调整项

**不包含：**
- 客户主数据（客户名称为纯文本字段）
- 审批流（订单无需审批，直接生效）
- 客户信用额管控
- 智能报价引擎（实时 BOM 成本计算）
- 订单变更影响分析/ATP 检查

## Design Decisions

| 决策 | 选择 | 原因 |
|------|------|------|
| 订单来源 | 可从报价单转、也可独立创建 | 灵活性，不强制走报价流程 |
| 订单审批 | 无需审批，直接生效 | 当前阶段简化流程 |
| 订单变更 | 主信息可改，行项目锁定 | 保护价格/数量约定 |
| 发货模式 | 多次分批发货 | 支持部分交付场景 |
| 发货扣库存 | 两步：申请确认 + 仓库出库 | 仓库操作独立于销售 |
| 退货关联 | 必须关联原发货单 | 可追溯，数量不超发货量 |
| 对账范围 | 发货 + 退货 + 调整项 | 全口径业财对齐 |

## Data Model

### sales_orders

```sql
CREATE TABLE sales_orders (
    order_id       BIGSERIAL PRIMARY KEY,
    order_no       VARCHAR(32) NOT NULL UNIQUE,
    quotation_id   BIGINT,
    customer_name  VARCHAR(200) NOT NULL,
    contact_person VARCHAR(100),
    contact_phone  VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    delivery_date  TIMESTAMPTZ,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

CREATE INDEX idx_sales_orders_status ON sales_orders(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_orders_customer ON sales_orders(customer_name) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_orders_quotation ON sales_orders(quotation_id) WHERE deleted_at IS NULL;
```

状态值：1=Draft, 2=Confirmed, 3=InProgress, 4=Completed, 5=Cancelled

### sales_order_items

```sql
CREATE TABLE sales_order_items (
    item_id       BIGSERIAL PRIMARY KEY,
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    unit_price    DECIMAL(14,6) NOT NULL,
    quantity      DECIMAL(14,6) NOT NULL,
    discount      DECIMAL(5,4) NOT NULL DEFAULT 1.0,
    subtotal      DECIMAL(14,2) NOT NULL,
    shipped_qty   DECIMAL(14,6) NOT NULL DEFAULT 0,
    returned_qty  DECIMAL(14,6) NOT NULL DEFAULT 0,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sales_order_items_order ON sales_order_items(order_id);
```

`shipped_qty` 在发货出库时累加，`returned_qty` 在退货完成时累加。发货时校验 `shipped_qty + 本次发货量 <= quantity`。

### shipping_requests

```sql
CREATE TABLE shipping_requests (
    request_id    BIGSERIAL PRIMARY KEY,
    request_no    VARCHAR(32) NOT NULL UNIQUE,
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    customer_name VARCHAR(200) NOT NULL,
    status        SMALLINT NOT NULL DEFAULT 1,
    remark        TEXT,
    operator_id   BIGINT,
    confirmed_at  TIMESTAMPTZ,
    shipped_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);

CREATE INDEX idx_shipping_requests_order ON shipping_requests(order_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_shipping_requests_status ON shipping_requests(status) WHERE deleted_at IS NULL;
```

状态值：1=Pending, 2=Confirmed, 3=Shipped, 4=Cancelled

### shipping_request_items

```sql
CREATE TABLE shipping_request_items (
    item_id       BIGSERIAL PRIMARY KEY,
    request_id    BIGINT NOT NULL REFERENCES shipping_requests(request_id),
    order_item_id BIGINT NOT NULL REFERENCES sales_order_items(item_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    quantity      DECIMAL(14,6) NOT NULL,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_shipping_request_items_request ON shipping_request_items(request_id);
```

### sales_returns

```sql
CREATE TABLE sales_returns (
    return_id     BIGSERIAL PRIMARY KEY,
    return_no     VARCHAR(32) NOT NULL UNIQUE,
    request_id    BIGINT NOT NULL REFERENCES shipping_requests(request_id),
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    customer_name VARCHAR(200) NOT NULL,
    status        SMALLINT NOT NULL DEFAULT 1,
    total_amount  DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark        TEXT,
    reason        TEXT,
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);

CREATE INDEX idx_sales_returns_request ON sales_returns(request_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_returns_order ON sales_returns(order_id) WHERE deleted_at IS NULL;
```

状态值：1=Pending, 2=Approved, 3=Received, 4=Completed, 5=Rejected

### sales_return_items

```sql
CREATE TABLE sales_return_items (
    item_id         BIGSERIAL PRIMARY KEY,
    return_id       BIGINT NOT NULL REFERENCES sales_returns(return_id),
    request_item_id BIGINT NOT NULL REFERENCES shipping_request_items(item_id),
    order_item_id   BIGINT NOT NULL REFERENCES sales_order_items(item_id),
    product_id      BIGINT NOT NULL,
    product_code    VARCHAR(100),
    product_name    VARCHAR(200),
    unit            VARCHAR(20),
    unit_price      DECIMAL(14,6) NOT NULL,
    quantity        DECIMAL(14,6) NOT NULL,
    subtotal        DECIMAL(14,2) NOT NULL,
    remark          TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sales_return_items_return ON sales_return_items(return_id);
```

退货数量校验：`退货数量 <= 发货数量 - 已退货数量`（按 order_item 维度）。

### reconciliation_statements

```sql
CREATE TABLE reconciliation_statements (
    statement_id     BIGSERIAL PRIMARY KEY,
    statement_no     VARCHAR(32) NOT NULL UNIQUE,
    customer_name    VARCHAR(200) NOT NULL,
    period_year      SMALLINT NOT NULL,
    period_month     SMALLINT NOT NULL,
    shipping_total   DECIMAL(14,2) NOT NULL DEFAULT 0,
    return_total     DECIMAL(14,2) NOT NULL DEFAULT 0,
    adjustment_total DECIMAL(14,2) NOT NULL DEFAULT 0,
    net_amount       DECIMAL(14,2) NOT NULL DEFAULT 0,
    status           SMALLINT NOT NULL DEFAULT 1,
    remark           TEXT,
    operator_id      BIGINT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at       TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_reconciliation_period ON reconciliation_statements(customer_name, period_year, period_month) WHERE deleted_at IS NULL;
```

状态值：1=Draft, 2=Confirmed, 3=Approved

`net_amount = shipping_total - return_total + adjustment_total`

### reconciliation_items

```sql
CREATE TABLE reconciliation_items (
    item_id       BIGSERIAL PRIMARY KEY,
    statement_id  BIGINT NOT NULL REFERENCES reconciliation_statements(statement_id),
    source_type   VARCHAR(20) NOT NULL,
    source_id     BIGINT,
    product_id    BIGINT,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    quantity      DECIMAL(14,6) NOT NULL,
    unit_price    DECIMAL(14,6) NOT NULL,
    amount        DECIMAL(14,2) NOT NULL,
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reconciliation_items_statement ON reconciliation_items(statement_id);
```

`source_type`：`shipping`（发货明细）、`return`（退货明细）、`adjustment`（手动调整项）。`amount` 正数表示发货/正调整，负数表示退货/负调整。

## Proto Definition

File: `proto/abt/v1/sales_order.proto`

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum SalesOrderStatus {
  SALES_ORDER_STATUS_UNSPECIFIED = 0;
  SALES_ORDER_STATUS_DRAFT = 1;
  SALES_ORDER_STATUS_CONFIRMED = 2;
  SALES_ORDER_STATUS_IN_PROGRESS = 3;
  SALES_ORDER_STATUS_COMPLETED = 4;
  SALES_ORDER_STATUS_CANCELLED = 5;
}

message SalesOrderItem {
  int64 item_id = 1;
  int64 order_id = 2;
  int64 product_id = 3;
  string product_code = 4;
  string product_name = 5;
  string unit = 6;
  string unit_price = 7;
  string quantity = 8;
  string discount = 9;
  string subtotal = 10;
  string shipped_qty = 11;
  string returned_qty = 12;
  string remark = 13;
}

message SalesOrder {
  int64 order_id = 1;
  string order_no = 2;
  int64 quotation_id = 3;
  string customer_name = 4;
  string contact_person = 5;
  string contact_phone = 6;
  SalesOrderStatus status = 7;
  string total_amount = 8;
  string remark = 9;
  int64 delivery_date = 10;
  int64 created_at = 11;
  int64 updated_at = 12;
  int64 operator_id = 13;
  repeated SalesOrderItem items = 14;
}

message CreateSalesOrderRequest {
  int64 quotation_id = 1;
  string customer_name = 2;
  string contact_person = 3;
  string contact_phone = 4;
  string remark = 5;
  int64 delivery_date = 6;
  repeated CreateSalesOrderItem items = 7;
}

message CreateSalesOrderItem {
  int64 product_id = 1;
  string unit_price = 2;
  string quantity = 3;
  string discount = 4;
  string remark = 5;
}

message UpdateSalesOrderRequest {
  int64 order_id = 1;
  string customer_name = 2;
  string contact_person = 3;
  string contact_phone = 4;
  string remark = 5;
  int64 delivery_date = 6;
}

message ListSalesOrdersRequest {
  optional string keyword = 1;
  optional SalesOrderStatus status = 2;
  optional PaginationParams pagination = 3;
}

message GetSalesOrderRequest {
  int64 order_id = 1;
}

message DeleteSalesOrderRequest {
  int64 order_id = 1;
}

message UpdateSalesOrderStatusRequest {
  int64 order_id = 1;
  SalesOrderStatus status = 2;
}

message SalesOrderResponse {
  SalesOrder order = 1;
}

message SalesOrderListResponse {
  repeated SalesOrder items = 1;
  PaginationInfo pagination = 2;
}

service SalesOrderService {
  rpc CreateSalesOrder(CreateSalesOrderRequest) returns (U64Response);
  rpc UpdateSalesOrder(UpdateSalesOrderRequest) returns (BoolResponse);
  rpc DeleteSalesOrder(DeleteSalesOrderRequest) returns (BoolResponse);
  rpc GetSalesOrder(GetSalesOrderRequest) returns (SalesOrderResponse);
  rpc ListSalesOrders(ListSalesOrdersRequest) returns (SalesOrderListResponse);
  rpc UpdateSalesOrderStatus(UpdateSalesOrderStatusRequest) returns (BoolResponse);
}
```

File: `proto/abt/v1/shipping.proto`

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum ShippingRequestStatus {
  SHIPPING_REQUEST_STATUS_UNSPECIFIED = 0;
  SHIPPING_REQUEST_STATUS_PENDING = 1;
  SHIPPING_REQUEST_STATUS_CONFIRMED = 2;
  SHIPPING_REQUEST_STATUS_SHIPPED = 3;
  SHIPPING_REQUEST_STATUS_CANCELLED = 4;
}

message ShippingRequestItem {
  int64 item_id = 1;
  int64 request_id = 2;
  int64 order_item_id = 3;
  int64 product_id = 4;
  string product_code = 5;
  string product_name = 6;
  string unit = 7;
  string quantity = 8;
  string remark = 9;
}

message ShippingRequest {
  int64 request_id = 1;
  string request_no = 2;
  int64 order_id = 3;
  string customer_name = 4;
  ShippingRequestStatus status = 5;
  string remark = 6;
  int64 operator_id = 7;
  int64 confirmed_at = 8;
  int64 shipped_at = 9;
  int64 created_at = 10;
  int64 updated_at = 11;
  repeated ShippingRequestItem items = 12;
}

message CreateShippingRequestRequest {
  int64 order_id = 1;
  string remark = 2;
  repeated CreateShippingRequestItem items = 3;
}

message CreateShippingRequestItem {
  int64 order_item_id = 1;
  string quantity = 2;
  string remark = 3;
}

message UpdateShippingRequestRequest {
  int64 request_id = 1;
  string remark = 2;
  repeated CreateShippingRequestItem items = 3;
}

message ListShippingRequestsRequest {
  optional string keyword = 1;
  optional ShippingRequestStatus status = 2;
  optional int64 order_id = 3;
  optional PaginationParams pagination = 4;
}

message GetShippingRequestRequest {
  int64 request_id = 1;
}

message UpdateShippingRequestStatusRequest {
  int64 request_id = 1;
  ShippingRequestStatus status = 2;
}

message DeleteShippingRequestRequest {
  int64 request_id = 1;
}

message ShippingRequestResponse {
  ShippingRequest request = 1;
}

message ShippingRequestListResponse {
  repeated ShippingRequest items = 1;
  PaginationInfo pagination = 2;
}

service ShippingRequestService {
  rpc CreateShippingRequest(CreateShippingRequestRequest) returns (U64Response);
  rpc UpdateShippingRequest(UpdateShippingRequestRequest) returns (BoolResponse);
  rpc DeleteShippingRequest(DeleteShippingRequestRequest) returns (BoolResponse);
  rpc GetShippingRequest(GetShippingRequestRequest) returns (ShippingRequestResponse);
  rpc ListShippingRequests(ListShippingRequestsRequest) returns (ShippingRequestListResponse);
  rpc UpdateShippingRequestStatus(UpdateShippingRequestStatusRequest) returns (BoolResponse);
}
```

File: `proto/abt/v1/sales_return.proto`

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum SalesReturnStatus {
  SALES_RETURN_STATUS_UNSPECIFIED = 0;
  SALES_RETURN_STATUS_PENDING = 1;
  SALES_RETURN_STATUS_APPROVED = 2;
  SALES_RETURN_STATUS_RECEIVED = 3;
  SALES_RETURN_STATUS_COMPLETED = 4;
  SALES_RETURN_STATUS_REJECTED = 5;
}

message SalesReturnItem {
  int64 item_id = 1;
  int64 return_id = 2;
  int64 request_item_id = 3;
  int64 order_item_id = 4;
  int64 product_id = 5;
  string product_code = 6;
  string product_name = 7;
  string unit = 8;
  string unit_price = 9;
  string quantity = 10;
  string subtotal = 11;
  string remark = 12;
}

message SalesReturn {
  int64 return_id = 1;
  string return_no = 2;
  int64 request_id = 3;
  int64 order_id = 4;
  string customer_name = 5;
  SalesReturnStatus status = 6;
  string total_amount = 7;
  string remark = 8;
  string reason = 9;
  int64 operator_id = 10;
  int64 created_at = 11;
  int64 updated_at = 12;
  repeated SalesReturnItem items = 13;
}

message CreateSalesReturnRequest {
  int64 request_id = 1;
  string remark = 2;
  string reason = 3;
  repeated CreateSalesReturnItem items = 4;
}

message CreateSalesReturnItem {
  int64 request_item_id = 1;
  string quantity = 2;
  string remark = 3;
}

message UpdateSalesReturnRequest {
  int64 return_id = 1;
  string remark = 2;
  string reason = 3;
  repeated CreateSalesReturnItem items = 4;
}

message ListSalesReturnsRequest {
  optional string keyword = 1;
  optional SalesReturnStatus status = 2;
  optional int64 order_id = 3;
  optional int64 request_id = 4;
  optional PaginationParams pagination = 5;
}

message GetSalesReturnRequest {
  int64 return_id = 1;
}

message UpdateSalesReturnStatusRequest {
  int64 return_id = 1;
  SalesReturnStatus status = 2;
}

message DeleteSalesReturnRequest {
  int64 return_id = 1;
}

message SalesReturnResponse {
  SalesReturn return_ = 1;
}

message SalesReturnListResponse {
  repeated SalesReturn items = 1;
  PaginationInfo pagination = 2;
}

service SalesReturnService {
  rpc CreateSalesReturn(CreateSalesReturnRequest) returns (U64Response);
  rpc UpdateSalesReturn(UpdateSalesReturnRequest) returns (BoolResponse);
  rpc DeleteSalesReturn(DeleteSalesReturnRequest) returns (BoolResponse);
  rpc GetSalesReturn(GetSalesReturnRequest) returns (SalesReturnResponse);
  rpc ListSalesReturns(ListSalesReturnsRequest) returns (SalesReturnListResponse);
  rpc UpdateSalesReturnStatus(UpdateSalesReturnStatusRequest) returns (BoolResponse);
}
```

File: `proto/abt/v1/reconciliation.proto`

```protobuf
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

enum ReconciliationStatus {
  RECONCILIATION_STATUS_UNSPECIFIED = 0;
  RECONCILIATION_STATUS_DRAFT = 1;
  RECONCILIATION_STATUS_CONFIRMED = 2;
  RECONCILIATION_STATUS_APPROVED = 3;
}

message ReconciliationItem {
  int64 item_id = 1;
  int64 statement_id = 2;
  string source_type = 3;
  int64 source_id = 4;
  int64 product_id = 5;
  string product_code = 6;
  string product_name = 7;
  string unit = 8;
  string quantity = 9;
  string unit_price = 10;
  string amount = 11;
  string remark = 12;
}

message ReconciliationStatement {
  int64 statement_id = 1;
  string statement_no = 2;
  string customer_name = 3;
  int32 period_year = 4;
  int32 period_month = 5;
  string shipping_total = 6;
  string return_total = 7;
  string adjustment_total = 8;
  string net_amount = 9;
  ReconciliationStatus status = 10;
  string remark = 11;
  int64 operator_id = 12;
  int64 created_at = 13;
  int64 updated_at = 14;
  repeated ReconciliationItem items = 15;
}

message CreateReconciliationRequest {
  string customer_name = 1;
  int32 period_year = 2;
  int32 period_month = 3;
  string remark = 4;
}

message AddReconciliationAdjustmentRequest {
  int64 statement_id = 1;
  repeated AdjustmentItem items = 2;
}

message AdjustmentItem {
  int64 product_id = 1;
  string quantity = 2;
  string unit_price = 3;
  string amount = 4;
  string remark = 5;
}

message UpdateReconciliationRequest {
  int64 statement_id = 1;
  string remark = 2;
}

message ListReconciliationsRequest {
  optional string keyword = 1;
  optional ReconciliationStatus status = 2;
  optional int32 period_year = 3;
  optional int32 period_month = 4;
  optional PaginationParams pagination = 5;
}

message GetReconciliationRequest {
  int64 statement_id = 1;
}

message UpdateReconciliationStatusRequest {
  int64 statement_id = 1;
  ReconciliationStatus status = 2;
}

message DeleteReconciliationRequest {
  int64 statement_id = 1;
}

message ReconciliationResponse {
  ReconciliationStatement statement = 1;
}

message ReconciliationListResponse {
  repeated ReconciliationStatement items = 1;
  PaginationInfo pagination = 2;
}

service ReconciliationService {
  rpc CreateReconciliation(CreateReconciliationRequest) returns (U64Response);
  rpc AddReconciliationAdjustment(AddReconciliationAdjustmentRequest) returns (BoolResponse);
  rpc UpdateReconciliation(UpdateReconciliationRequest) returns (BoolResponse);
  rpc DeleteReconciliation(DeleteReconciliationRequest) returns (BoolResponse);
  rpc GetReconciliation(GetReconciliationRequest) returns (ReconciliationResponse);
  rpc ListReconciliations(ListReconciliationsRequest) returns (ReconciliationListResponse);
  rpc UpdateReconciliationStatus(UpdateReconciliationStatusRequest) returns (BoolResponse);
}
```

## Rust Models

File: `abt/src/models/sales_order.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesOrder {
    pub order_id: i64,
    pub order_no: String,
    pub quotation_id: Option<i64>,
    pub customer_name: String,
    pub contact_person: Option<String>,
    pub contact_phone: Option<String>,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub delivery_date: Option<DateTime<Utc>>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub items: Vec<SalesOrderItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesOrderItem {
    pub item_id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub subtotal: Decimal,
    pub shipped_qty: Decimal,
    pub returned_qty: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SalesOrderQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

File: `abt/src/models/shipping_request.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShippingRequest {
    pub request_id: i64,
    pub request_no: String,
    pub order_id: i64,
    pub customer_name: String,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub items: Vec<ShippingRequestItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShippingRequestItem {
    pub item_id: i64,
    pub request_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShippingRequestQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub order_id: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

File: `abt/src/models/sales_return.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesReturn {
    pub return_id: i64,
    pub return_no: String,
    pub request_id: i64,
    pub order_id: i64,
    pub customer_name: String,
    pub status: i16,
    pub total_amount: Decimal,
    pub remark: Option<String>,
    pub reason: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub items: Vec<SalesReturnItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SalesReturnItem {
    pub item_id: i64,
    pub return_id: i64,
    pub request_item_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SalesReturnQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub order_id: Option<i64>,
    pub request_id: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

File: `abt/src/models/reconciliation.rs`

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReconciliationStatement {
    pub statement_id: i64,
    pub statement_no: String,
    pub customer_name: String,
    pub period_year: i16,
    pub period_month: i16,
    pub shipping_total: Decimal,
    pub return_total: Decimal,
    pub adjustment_total: Decimal,
    pub net_amount: Decimal,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub items: Vec<ReconciliationItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReconciliationItem {
    pub item_id: i64,
    pub statement_id: i64,
    pub source_type: String,
    pub source_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ReconciliationQuery {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub period_year: Option<i16>,
    pub period_month: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
```

## Repository Layer

### SalesOrderRepo

```rust
impl SalesOrderRepo {
    pub async fn insert(executor: Executor<'_>, order: &SalesOrder) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, order: &SalesOrder) -> Result<()>;
    pub async fn update_header(executor: Executor<'_>, order_id: i64, customer_name: &str, contact_person: Option<&str>, contact_phone: Option<&str>, remark: Option<&str>, delivery_date: Option<DateTime<Utc>>) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, order_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, order_id: i64) -> Result<Option<SalesOrder>>;
    pub async fn query(pool: &PgPool, query: &SalesOrderQuery) -> Result<Vec<SalesOrder>>;
    pub async fn query_count(pool: &PgPool, query: &SalesOrderQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, order_id: i64, status: i16) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[SalesOrderItem]) -> Result<()>;
    pub async fn find_by_order_id(pool: &PgPool, order_id: i64) -> Result<Vec<SalesOrderItem>>;
    pub async fn update_shipped_qty(executor: Executor<'_>, item_id: i64, qty: Decimal) -> Result<()>;
    pub async fn update_returned_qty(executor: Executor<'_>, item_id: i64, qty: Decimal) -> Result<()>;
}
```

### ShippingRequestRepo

```rust
impl ShippingRequestRepo {
    pub async fn insert(executor: Executor<'_>, request: &ShippingRequest) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, request: &ShippingRequest) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, request_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, request_id: i64) -> Result<Option<ShippingRequest>>;
    pub async fn query(pool: &PgPool, query: &ShippingRequestQuery) -> Result<Vec<ShippingRequest>>;
    pub async fn query_count(pool: &PgPool, query: &ShippingRequestQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, request_id: i64, status: i16) -> Result<()>;
    pub async fn update_shipped_at(executor: Executor<'_>, request_id: i64) -> Result<()>;
    pub async fn update_confirmed_at(executor: Executor<'_>, request_id: i64) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[ShippingRequestItem]) -> Result<()>;
    pub async fn delete_by_request(executor: Executor<'_>, request_id: i64) -> Result<()>;
    pub async fn find_by_request_id(pool: &PgPool, request_id: i64) -> Result<Vec<ShippingRequestItem>>;
}
```

### SalesReturnRepo

```rust
impl SalesReturnRepo {
    pub async fn insert(executor: Executor<'_>, ret: &SalesReturn) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, ret: &SalesReturn) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, return_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, return_id: i64) -> Result<Option<SalesReturn>>;
    pub async fn query(pool: &PgPool, query: &SalesReturnQuery) -> Result<Vec<SalesReturn>>;
    pub async fn query_count(pool: &PgPool, query: &SalesReturnQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, return_id: i64, status: i16) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[SalesReturnItem]) -> Result<()>;
    pub async fn delete_by_return(executor: Executor<'_>, return_id: i64) -> Result<()>;
    pub async fn find_by_return_id(pool: &PgPool, return_id: i64) -> Result<Vec<SalesReturnItem>>;
}
```

### ReconciliationRepo

```rust
impl ReconciliationRepo {
    pub async fn insert(executor: Executor<'_>, statement: &ReconciliationStatement) -> Result<i64>;
    pub async fn update(executor: Executor<'_>, statement: &ReconciliationStatement) -> Result<()>;
    pub async fn soft_delete(executor: Executor<'_>, statement_id: i64) -> Result<()>;
    pub async fn find_by_id(pool: &PgPool, statement_id: i64) -> Result<Option<ReconciliationStatement>>;
    pub async fn query(pool: &PgPool, query: &ReconciliationQuery) -> Result<Vec<ReconciliationStatement>>;
    pub async fn query_count(pool: &PgPool, query: &ReconciliationQuery) -> Result<i64>;
    pub async fn update_status(executor: Executor<'_>, statement_id: i64, status: i16) -> Result<()>;
    pub async fn insert_items(executor: Executor<'_>, items: &[ReconciliationItem]) -> Result<()>;
    pub async fn find_by_statement_id(pool: &PgPool, statement_id: i64) -> Result<Vec<ReconciliationItem>>;
    pub async fn delete_adjustments_by_statement(executor: Executor<'_>, statement_id: i64) -> Result<()>;
    pub async fn recalculate_totals(executor: Executor<'_>, statement_id: i64) -> Result<()>;
}
```

## Service Layer

### Traits

```rust
#[async_trait]
pub trait SalesOrderService {
    async fn create(&self, operator_id: Option<i64>, order: SalesOrder, executor: Executor<'_>) -> Result<i64>;
    async fn update_header(&self, order_id: i64, customer_name: String, contact_person: Option<String>, contact_phone: Option<String>, remark: Option<String>, delivery_date: Option<DateTime<Utc>>) -> Result<()>;
    async fn delete(&self, order_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, order_id: i64) -> Result<Option<SalesOrder>>;
    async fn list(&self, query: SalesOrderQuery) -> Result<PaginatedResult<SalesOrder>>;
    async fn update_status(&self, order_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait ShippingRequestService {
    async fn create(&self, operator_id: Option<i64>, request: ShippingRequest, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, request: ShippingRequest, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, request_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, request_id: i64) -> Result<Option<ShippingRequest>>;
    async fn list(&self, query: ShippingRequestQuery) -> Result<PaginatedResult<ShippingRequest>>;
    async fn update_status(&self, request_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait SalesReturnService {
    async fn create(&self, operator_id: Option<i64>, ret: SalesReturn, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, ret: SalesReturn, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, return_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, return_id: i64) -> Result<Option<SalesReturn>>;
    async fn list(&self, query: SalesReturnQuery) -> Result<PaginatedResult<SalesReturn>>;
    async fn update_status(&self, return_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}

#[async_trait]
pub trait ReconciliationService {
    async fn create(&self, operator_id: Option<i64>, statement: ReconciliationStatement, executor: Executor<'_>) -> Result<i64>;
    async fn add_adjustments(&self, statement_id: i64, adjustments: Vec<ReconciliationItem>, executor: Executor<'_>) -> Result<()>;
    async fn update(&self, statement_id: i64, remark: Option<String>) -> Result<()>;
    async fn delete(&self, statement_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, statement_id: i64) -> Result<Option<ReconciliationStatement>>;
    async fn list(&self, query: ReconciliationQuery) -> Result<PaginatedResult<ReconciliationStatement>>;
    async fn update_status(&self, statement_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
```

### Business Logic

**SalesOrderServiceImpl:**

- **create**: 若提供 `quotation_id`，校验报价单为 Accepted 状态，复制行项目（价格/产品/折扣）。否则独立创建。生成编号 SOYYYYMMNNNNN。校验 product_id 存在性，冗余写入产品信息。计算 subtotal/total_amount。初始状态 Draft。
- **update_header**: 校验订单存在。Confirmed 及之后状态允许修改主信息（不涉及行项目）。
- **delete**: 仅 Draft 可删。软删除。
- **update_status**: Draft→Confirmed→Cancelled, Confirmed→InProgress→Completed。首次发货确认时自动从 Confirmed 转为 InProgress。

**ShippingRequestServiceImpl:**

- **create**: 校验订单为 Confirmed 或 InProgress 状态。每行的 `quantity <= order_item.quantity - order_item.shipped_qty`。从 order_item 冗余产品信息。生成编号 SRYYYYMMNNNNN。初始状态 Pending。
- **update**: 仅 Pending 可改行项目。重新校验数量约束。
- **update_status**:
  - Pending→Confirmed: 记录 confirmed_at
  - Confirmed→Shipped: 记录 shipped_at，累加 `order_item.shipped_qty`，调用库存出库
  - Pending→Cancelled: 可取消
- **delete**: 仅 Pending 可删。

**SalesReturnServiceImpl:**

- **create**: 校验发货单为 Shipped 状态。每行 `quantity <= shipped_qty - 已退货数量`（按 order_item 维度）。从 shipping_request_item 获取产品信息和原价。生成编号 RTYYYYMMNNNNN。初始状态 Pending。
- **update**: 仅 Pending 可改。
- **update_status**:
  - Pending→Approved→Received→Completed
  - Pending/Approved→Rejected
  - Completed 时累加 `order_item.returned_qty`，调用库存入库（退货入库）

**ReconciliationServiceImpl:**

- **create**: 指定客户 + 年月，查询该客户该月所有 Shipped 状态的发货单明细和 Completed 状态的退货单明细，生成 reconciliation_items（source_type=shipping/return）。计算 shipping_total、return_total、net_amount。生成编号 RCYYYYMMNNNNN。同一客户同月不允许重复创建（unique index）。
- **add_adjustments**: 仅 Draft 状态可添加调整项。先删除旧调整项（source_type=adjustment），重新插入。调用 recalculate_totals 重算 adjustment_total 和 net_amount。
- **update_status**: Draft→Confirmed→Approved。仅 Draft 可改 remark。
- **delete**: 仅 Draft 可删。

### Inventory Integration

发货出库和退货入库通过调用现有库存服务的出库/入库方法实现。集成点在 ShippingRequestServiceImpl 和 SalesReturnServiceImpl 的状态转换中：

- Shipped 状态转换时：调用库存出库接口，传入产品 ID、数量、库位等信息
- Return Completed 时：调用库存入库接口，退货入库

具体集成方式取决于现有库存模块的 Service API，在实现阶段对接。

## Status Enum Mapping

### SalesOrderStatus

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SALES_ORDER_STATUS_DRAFT | 草稿 |
| 2 | SALES_ORDER_STATUS_CONFIRMED | 已确认 |
| 3 | SALES_ORDER_STATUS_IN_PROGRESS | 进行中 |
| 4 | SALES_ORDER_STATUS_COMPLETED | 已完成 |
| 5 | SALES_ORDER_STATUS_CANCELLED | 已取消 |

### ShippingRequestStatus

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SHIPPING_REQUEST_STATUS_PENDING | 待确认 |
| 2 | SHIPPING_REQUEST_STATUS_CONFIRMED | 已确认 |
| 3 | SHIPPING_REQUEST_STATUS_SHIPPED | 已出库 |
| 4 | SHIPPING_REQUEST_STATUS_CANCELLED | 已取消 |

### SalesReturnStatus

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SALES_RETURN_STATUS_PENDING | 待审核 |
| 2 | SALES_RETURN_STATUS_APPROVED | 已审核 |
| 3 | SALES_RETURN_STATUS_RECEIVED | 已收货 |
| 4 | SALES_RETURN_STATUS_COMPLETED | 已完成 |
| 5 | SALES_RETURN_STATUS_REJECTED | 已拒绝 |

### ReconciliationStatus

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | RECONCILIATION_STATUS_DRAFT | 草稿 |
| 2 | RECONCILIATION_STATUS_CONFIRMED | 已确认 |
| 3 | RECONCILIATION_STATUS_APPROVED | 已审批 |

## File List

| Layer | New Files |
|-------|-----------|
| Proto | `proto/abt/v1/sales_order.proto`, `proto/abt/v1/shipping.proto`, `proto/abt/v1/sales_return.proto`, `proto/abt/v1/reconciliation.proto` |
| Model | `abt/src/models/sales_order.rs`, `abt/src/models/shipping_request.rs`, `abt/src/models/sales_return.rs`, `abt/src/models/reconciliation.rs` |
| Repository | `abt/src/repositories/sales_order_repo.rs`, `abt/src/repositories/shipping_request_repo.rs`, `abt/src/repositories/sales_return_repo.rs`, `abt/src/repositories/reconciliation_repo.rs` |
| Service | `abt/src/service/sales_order_service.rs`, `abt/src/service/shipping_request_service.rs`, `abt/src/service/sales_return_service.rs`, `abt/src/service/reconciliation_service.rs` |
| Impl | `abt/src/implt/sales_order_service_impl.rs`, `abt/src/implt/shipping_request_service_impl.rs`, `abt/src/implt/sales_return_service_impl.rs`, `abt/src/implt/reconciliation_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/sales_order.rs`, `abt-grpc/src/handlers/shipping_request.rs`, `abt-grpc/src/handlers/sales_return.rs`, `abt-grpc/src/handlers/reconciliation.rs` |
| Migration | `abt/migrations/XXX_create_sales_orders.sql`, `abt/migrations/XXX_create_shipping_requests.sql`, `abt/migrations/XXX_create_sales_returns.sql`, `abt/migrations/XXX_create_reconciliation.sql` |

## Registration

- `abt/src/lib.rs`: 添加 `get_sales_order_service`, `get_shipping_request_service`, `get_sales_return_service`, `get_reconciliation_service` 工厂函数
- `abt-grpc/src/server.rs`: 注册 4 个 ServiceServer
- `document_sequences` 表初始化 4 条序列记录：SO/SR/RT/RC 前缀
