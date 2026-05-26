---
name: shipping-request
description: 发货申请模块设计，含创建/确认/发货流程、数量校验、库存出库联动和订单行已发量累加
---

# Shipping Request Module Design

Date: 2026-05-21

## Overview

发货申请是销售订单到实物出库的桥梁。销售创建发货申请 → 仓库确认 → 发货出库，形成完整的发货闭环。发货时自动调用库存出库并累加订单行的已发货数量。

## Scope

**包含：**
- 发货申请 CRUD（关联销售订单）
- 发货数量校验（不超过订单行剩余可发量）
- 状态流转（Pending → Confirmed → Shipped / Cancelled）
- 发货确认时自动确认时间戳
- 发货出库时：库存出库 + 订单行 shipped_qty 累加
- 订单状态自动推进（Confirmed → InProgress）

**不包含：**
- 物流信息跟踪
- 发货单打印
- 库位指定（location_id 占位为 0）

## Data Model

### shipping_requests

```sql
CREATE TABLE shipping_requests (
    request_id    BIGSERIAL PRIMARY KEY,
    request_no    VARCHAR(32) NOT NULL UNIQUE,       -- SR-YYYY-MM-NNNNN
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    customer_name VARCHAR(200) NOT NULL,             -- 从订单冗余
    status        SMALLINT NOT NULL DEFAULT 1,       -- 1=待确认,2=已确认,3=已发货,4=已取消
    remark        TEXT,
    operator_id   BIGINT,
    confirmed_at  TIMESTAMPTZ,                       -- 确认时间
    shipped_at    TIMESTAMPTZ,                       -- 发货时间
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);
```

### shipping_request_items

```sql
CREATE TABLE shipping_request_items (
    item_id       BIGSERIAL PRIMARY KEY,
    request_id    BIGINT NOT NULL REFERENCES shipping_requests(request_id),
    order_item_id BIGINT NOT NULL REFERENCES sales_order_items(item_id),
    product_id    BIGINT NOT NULL,                   -- 从订单行冗余
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    quantity      DECIMAL(14,6) NOT NULL,            -- 本次发货数量
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Proto Definition

File: `proto/abt/v1/shipping.proto`

6 个 RPC：
- `CreateShippingRequest` — 创建发货申请（指定 order_id + 行项目）
- `UpdateShippingRequest` — 更新（Pending 状态可改 remark + items）
- `DeleteShippingRequest` — 软删除（仅 Pending）
- `GetShippingRequest` — 按ID查询
- `ListShippingRequests` — 分页查询（支持 order_id 过滤）
- `UpdateShippingRequestStatus` — 状态变更

Key decisions:
- `order_item_id` 关联到订单行项目，用于数量校验和 shipped_qty 累加
- 产品信息从订单行项目自动填充，不在请求中指定
- `ListShippingRequestsRequest` 支持 `order_id` 过滤，方便查看某订单的所有发货记录

## Business Logic

### create
1. 校验关联订单状态为 Confirmed(2) 或 InProgress(3)
2. 获取订单行项目
3. 逐行校验发货数量 ≤ 订单行剩余可发量（`quantity - shipped_qty`）
4. 从订单行填充产品信息（product_id/code/name/unit）
5. 调用 `DocumentSequenceRepo::next_number(executor, "SR")` 生成编号
6. 从订单冗余 `customer_name`
7. 插入主表 + 行项目

### update
- 仅 Pending(1) 状态可编辑
- 重新校验数量并填充产品信息
- 整体替换行项目（先删后插）

### delete
- 仅 Pending(1) 状态可删除

### update_status

状态转换及副作用：

| From → To | 含义 | 副作用 |
|-----------|------|--------|
| Pending(1) → Confirmed(2) | 确认 | 设置 `confirmed_at` |
| Pending(1) → Cancelled(4) | 取消 | 无 |
| Confirmed(2) → Shipped(3) | 发货 | 设置 `shipped_at`；逐行调用 `InventoryService::stock_out`；累加 `SalesOrderRepo::update_shipped_qty`；若订单为 Confirmed 则推进为 InProgress(3) |

### 库存联动

发货时对每个行项目调用 `InventoryService::stock_out(StockChangeRequest)`：
- `operation_type = Out`
- `ref_order_type = "shipping_request"`
- `ref_order_id = request_no`
- `location_id = 0`（占位，待后续集成具体库位）

## Status Enum Mapping

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SHIPPING_REQUEST_STATUS_PENDING | 待确认 |
| 2 | SHIPPING_REQUEST_STATUS_CONFIRMED | 已确认 |
| 3 | SHIPPING_REQUEST_STATUS_SHIPPED | 已发货 |
| 4 | SHIPPING_REQUEST_STATUS_CANCELLED | 已取消 |

## File List

| Layer | Files |
|-------|-------|
| Proto | `proto/abt/v1/shipping.proto` |
| Migration | `abt/migrations/048_create_shipping_requests.sql` |
| Model | `abt/src/models/shipping_request.rs` |
| Repository | `abt/src/repositories/shipping_request_repo.rs` |
| Service | `abt/src/service/shipping_request_service.rs` |
| Impl | `abt/src/implt/shipping_request_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/shipping_request.rs` |
