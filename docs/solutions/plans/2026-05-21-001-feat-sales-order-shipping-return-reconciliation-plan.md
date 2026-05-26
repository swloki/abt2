---
name: sales-order-shipping-return-reconciliation-plan
description: 销售订单/发货/退货/对账模块实现计划，与设计文档对应
---

# Sales Order / Shipping / Return / Reconciliation Implementation Plan

Date: 2026-05-21

## Overview

本计划覆盖销售管理模块中除报价单外的 4 个子模块：销售订单、发货申请、销售退货、对账单。报价单已在独立计划中完成。

设计文档：
- [Sales Order Design](2026-05-21-sales-order-design.md)
- [Shipping Request Design](2026-05-21-shipping-request-design.md)
- [Sales Return Design](2026-05-21-sales-return-design.md)
- [Reconciliation Design](2026-05-21-reconciliation-design.md)

## Dependencies

- 文档编号服务（已在 Quotation 模块中实现）
- 库存服务（`InventoryService::stock_in / stock_out`，已有）

## Implementation Steps

### Step 1: Proto Definitions + Migrations

**Proto 文件：**
- `proto/abt/v1/sales_order.proto` — SalesOrderService（6 RPC）
- `proto/abt/v1/shipping.proto` — ShippingRequestService（6 RPC）
- `proto/abt/v1/sales_return.proto` — SalesReturnService（6 RPC）
- `proto/abt/v1/reconciliation.proto` — ReconciliationService（7 RPC）

**数据库迁移：**
- `047_create_sales_orders.sql` — sales_orders + sales_order_items
- `048_create_shipping_requests.sql` — shipping_requests + shipping_request_items
- `049_create_sales_returns.sql` — sales_returns + sales_return_items
- `050_create_reconciliation.sql` — reconciliation_statements + reconciliation_items（含唯一索引 customer_name + period_year + period_month）
- `051_seed_sales_sequences.sql` — SO/SR/RT/RC 四条文档编号序列

### Step 2: Models + Repositories

| 模块 | Model 文件 | Repo 文件 |
|------|-----------|----------|
| Sales Order | `models/sales_order.rs` — SalesOrder, SalesOrderItem, SalesOrderQuery | `repositories/sales_order_repo.rs` — CRUD + update_shipped_qty + update_returned_qty |
| Shipping | `models/shipping_request.rs` — ShippingRequest, ShippingRequestItem, ShippingRequestQuery | `repositories/shipping_request_repo.rs` — CRUD + update_confirmed_at + update_shipped_at |
| Return | `models/sales_return.rs` — SalesReturn, SalesReturnItem, SalesReturnQuery | `repositories/sales_return_repo.rs` — CRUD + sum_returned_qty |
| Reconciliation | `models/reconciliation.rs` — ReconciliationStatement, ReconciliationItem, ReconciliationQuery | `repositories/reconciliation_repo.rs` — CRUD + query_shipping_items + query_return_items + delete_adjustments + update_totals |

### Step 3: Service Traits + Impls

| 模块 | Service Trait | Service Impl |
|------|-------------|-------------|
| Sales Order | 6 方法：create, update_header, delete, get_by_id, list, update_status | 报价单转订单 + 产品校验 + 状态白名单 |
| Shipping | 6 方法：create, update, delete, get_by_id, list, update_status | 数量校验 + 发货时库存出库 + shipped_qty 累加 |
| Return | 6 方法：create, update, delete, get_by_id, list, update_status | 发货单校验 + 可退量校验 + 完成时库存入库 + returned_qty 累加 |
| Reconciliation | 7 方法：create, add_adjustments, update, delete, get_by_id, list, update_status | 自动汇总发货/退货明细 + 调整项替换 + 汇总额重算 |

### Step 4: gRPC Handlers + Registration

| Handler 文件 | 注册 |
|-------------|------|
| `handlers/sales_order.rs` | `server.rs` — SalesOrderServiceServer |
| `handlers/shipping_request.rs` | `server.rs` — ShippingRequestServiceServer |
| `handlers/sales_return.rs` | `server.rs` — SalesReturnServiceServer |
| `handlers/reconciliation.rs` | `server.rs` — ReconciliationServiceServer |

`lib.rs` 新增工厂函数：`get_sales_order_service` / `get_shipping_request_service` / `get_sales_return_service` / `get_reconciliation_service`

`server.rs` AppState 新增对应 service 方法。

## Status

全部已实现。Clippy 通过。
