---
name: sales-order
description: 销售订单模块设计，包含订单 CRUD、报价单转订单、状态流转和发货/退货跟踪
---

# Sales Order Module Design

Date: 2026-05-21

## Overview

销售订单是销售管理的核心单据，承接报价单（可选）或独立创建。管理从订单创建到完成的完整生命周期，同时跟踪每行的发货和退货数量。

## Scope

**包含：**
- 销售订单 CRUD（主表 + 行项目）
- 报价单转订单（基于已接受的报价单自动填充行项目）
- 订单状态流转（Draft → Confirmed → InProgress → Completed / Cancelled）
- 行项目发货/退货数量跟踪（`shipped_qty` / `returned_qty`）

**不包含：**
- 订单行项目的增删改（仅支持 UpdateHeader）
- 库存联动（由 ShippingRequest 模块负责）
- 价格自动计算（手动填写，不联动 BOM 成本）

## Data Model

### sales_orders

```sql
CREATE TABLE sales_orders (
    order_id       BIGSERIAL PRIMARY KEY,
    order_no       VARCHAR(32) NOT NULL UNIQUE,      -- SO-YYYY-MM-NNNNN
    quotation_id   BIGINT,                           -- 关联报价单，可为空
    customer_name  VARCHAR(200) NOT NULL,
    contact_person VARCHAR(100),
    contact_phone  VARCHAR(50),
    status         SMALLINT NOT NULL DEFAULT 1,      -- 1=草稿,2=已确认,3=进行中,4=已完成,5=已取消
    total_amount   DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark         TEXT,
    delivery_date  TIMESTAMPTZ,
    operator_id    BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ                       -- 软删除
);

CREATE INDEX idx_sales_orders_status ON sales_orders(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_orders_customer ON sales_orders(customer_name) WHERE deleted_at IS NULL;
CREATE INDEX idx_sales_orders_quotation ON sales_orders(quotation_id) WHERE deleted_at IS NULL;
```

### sales_order_items

```sql
CREATE TABLE sales_order_items (
    item_id       BIGSERIAL PRIMARY KEY,
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    product_id    BIGINT NOT NULL,
    product_code  VARCHAR(100),                      -- 冗余存储，避免产品改名影响历史
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    unit_price    DECIMAL(14,6) NOT NULL,
    quantity      DECIMAL(14,6) NOT NULL,
    discount      DECIMAL(5,4) NOT NULL DEFAULT 1.0, -- 折扣率 0~1
    subtotal      DECIMAL(14,2) NOT NULL,             -- = unit_price * quantity * discount
    shipped_qty   DECIMAL(14,6) NOT NULL DEFAULT 0,  -- 由 ShippingRequest 累加
    returned_qty  DECIMAL(14,6) NOT NULL DEFAULT 0,  -- 由 SalesReturn 累加
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

行项目无独立软删除，随主表级联。

## Proto Definition

File: `proto/abt/v1/sales_order.proto`

6 个 RPC：
- `CreateSalesOrder` — 创建订单（可关联 quotation_id）
- `UpdateSalesOrder` — 更新订单头部（不含行项目）
- `DeleteSalesOrder` — 软删除（仅 Draft）
- `GetSalesOrder` — 按ID查询（含行项目）
- `ListSalesOrders` — 分页查询（keyword/status 过滤）
- `UpdateSalesOrderStatus` — 状态变更

Key decisions:
- `order_no` 系统自动生成，调用文档编号服务（doc_type="SO"）
- `CreateSalesOrderRequest.items` 创建时指定行项目，`UpdateSalesOrderRequest` 不含 items（仅更新头部）
- `shipped_qty` / `returned_qty` 为只读字段，由下游模块（发货/退货）自动更新
- `quotation_id` 可选：传入已接受的报价单 ID 时，自动从报价单复制行项目

## Business Logic

### create
1. 若传入 `quotation_id`：校验报价单状态为 Accepted(3)，复制其行项目
2. 校验所有 `product_id` 存在性
3. 调用 `DocumentSequenceRepo::next_number(executor, "SO")` 生成编号
4. 计算每行 subtotal，聚合 total_amount
5. 插入主表 + 批量插入行项目

### update_header
- 不校验状态（任何状态均可修改头部信息）

### delete
- 仅 Draft(1) 状态可删除

### update_status

状态转换白名单：

| From | To | 含义 |
|------|----|------|
| Draft(1) | Confirmed(2) | 确认订单 |
| Draft(1) | Cancelled(5) | 取消 |
| Confirmed(2) | InProgress(3) | 开始执行（由发货触发） |
| InProgress(3) | Completed(4) | 完结 |

## Status Enum Mapping

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SALES_ORDER_STATUS_DRAFT | 草稿 |
| 2 | SALES_ORDER_STATUS_CONFIRMED | 已确认 |
| 3 | SALES_ORDER_STATUS_IN_PROGRESS | 进行中 |
| 4 | SALES_ORDER_STATUS_COMPLETED | 已完成 |
| 5 | SALES_ORDER_STATUS_CANCELLED | 已取消 |

## File List

| Layer | Files |
|-------|-------|
| Proto | `proto/abt/v1/sales_order.proto` |
| Migration | `abt/migrations/047_create_sales_orders.sql` |
| Model | `abt/src/models/sales_order.rs` |
| Repository | `abt/src/repositories/sales_order_repo.rs` |
| Service | `abt/src/service/sales_order_service.rs` |
| Impl | `abt/src/implt/sales_order_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/sales_order.rs` |
