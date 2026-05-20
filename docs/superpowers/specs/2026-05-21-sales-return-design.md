---
name: sales-return
description: 销售退货模块设计，含退货申请、数量校验、退货入库联动和订单行已退量累加
---

# Sales Return Module Design

Date: 2026-05-21

## Overview

销售退货管理客户退回商品的完整流程。退货基于已发货的发货申请创建，经审批后完成退货入库。退货完成时自动调用库存入库并累加订单行的已退货数量。

## Scope

**包含：**
- 退货单 CRUD（关联发货申请 + 销售订单）
- 退货数量校验（不超过发货行剩余可退量，考虑同状态的其他退货单）
- 状态流转（Pending → Approved → Received → Completed / Rejected）
- 退货完成时：库存入库 + 订单行 returned_qty 累加
- 退货原因记录

**不包含：**
- 退款处理
- 质检流程
- 库位指定（location_id 占位为 0）

## Data Model

### sales_returns

```sql
CREATE TABLE sales_returns (
    return_id     BIGSERIAL PRIMARY KEY,
    return_no     VARCHAR(32) NOT NULL UNIQUE,       -- RT-YYYY-MM-NNNNN
    request_id    BIGINT NOT NULL REFERENCES shipping_requests(request_id),
    order_id      BIGINT NOT NULL REFERENCES sales_orders(order_id),
    customer_name VARCHAR(200) NOT NULL,             -- 从发货申请冗余
    status        SMALLINT NOT NULL DEFAULT 1,       -- 1=待处理,2=已确认,3=已入库,4=已完成,5=已拒绝
    total_amount  DECIMAL(14,2) NOT NULL DEFAULT 0,
    remark        TEXT,
    reason        TEXT,                               -- 退货原因
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);
```

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
    unit_price      DECIMAL(14,6) NOT NULL,          -- 从订单行取单价
    quantity        DECIMAL(14,6) NOT NULL,
    subtotal        DECIMAL(14,2) NOT NULL,           -- = unit_price * quantity
    remark          TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Proto Definition

File: `proto/abt/v1/sales_return.proto`

6 个 RPC：
- `CreateSalesReturn` — 创建退货（指定 request_id + 行项目）
- `UpdateSalesReturn` — 更新（Pending 状态可改 remark/reason + items）
- `DeleteSalesReturn` — 软删除（仅 Pending）
- `GetSalesReturn` — 按ID查询
- `ListSalesReturns` — 分页查询（支持 order_id / request_id 过滤）
- `UpdateSalesReturnStatus` — 状态变更

Key decisions:
- `request_id` 关联已发货的发货申请
- `request_item_id` 关联发货行项目，用于数量校验
- `order_item_id` 冗余存储，用于 returned_qty 累加
- `unit_price` 从订单行项目取得，自动计算 subtotal
- `total_amount` = sum(subtotal)，退货金额使用订单原价

## Business Logic

### create
1. 校验发货申请存在且状态为 Shipped(3)
2. 获取发货行项目和订单行项目
3. 逐行校验：
   - 发货行存在（request_item_id 有效）
   - 退货数量 ≤ 发货行剩余可退量（`ship_qty - sum_returned_qty`）
   - `sum_returned_qty` 包含 Pending/Approved/Received/Completed 状态的所有退货
4. 从发货行填充产品信息，从订单行取 unit_price
5. 计算 subtotal 和 total_amount
6. 调用 `DocumentSequenceRepo::next_number(executor, "RT")` 生成编号
7. 从发货申请冗余 `order_id`、`customer_name`

### update
- 仅 Pending(1) 状态可编辑
- 重新校验数量并填充信息
- 整体替换行项目（先删后插）

### delete
- 仅 Pending(1) 状态可删除

### update_status

状态转换及副作用：

| From → To | 含义 | 副作用 |
|-----------|------|--------|
| Pending(1) → Approved(2) | 审批通过 | 无 |
| Approved(2) → Received(3) | 收货确认 | 无 |
| Received(3) → Completed(4) | 完成退货 | 逐行调用 `InventoryService::stock_in`；累加 `SalesOrderRepo::update_returned_qty` |
| Pending(1) → Rejected(5) | 拒绝 | 无 |
| Approved(2) → Rejected(5) | 拒绝 | 无 |

### 库存联动

退货完成时对每个行项目调用 `InventoryService::stock_in(StockChangeRequest)`：
- `operation_type = In`
- `ref_order_type = "sales_return"`
- `ref_order_id = return_no`
- `location_id = 0`（占位，待后续集成具体库位）

## Status Enum Mapping

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | SALES_RETURN_STATUS_PENDING | 待处理 |
| 2 | SALES_RETURN_STATUS_APPROVED | 已确认 |
| 3 | SALES_RETURN_STATUS_RECEIVED | 已入库 |
| 4 | SALES_RETURN_STATUS_COMPLETED | 已完成 |
| 5 | SALES_RETURN_STATUS_REJECTED | 已拒绝 |

## File List

| Layer | Files |
|-------|-------|
| Proto | `proto/abt/v1/sales_return.proto` |
| Migration | `abt/migrations/049_create_sales_returns.sql` |
| Model | `abt/src/models/sales_return.rs` |
| Repository | `abt/src/repositories/sales_return_repo.rs` |
| Service | `abt/src/service/sales_return_service.rs` |
| Impl | `abt/src/implt/sales_return_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/sales_return.rs` |
