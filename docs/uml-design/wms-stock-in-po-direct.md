# 采购入库 PO 直收设计（取消来料通知后）

> 取代 `wms-stock-in-unified.md` 的「来料通知闭环」路径。工厂不做来料质检，来料通知为冗余单据，PO 直接收货入库。
> 状态：**已完成**（2026-06，第一步 + 2a + 2b 全部落地）。`PurchaseStockInService` PO 直收入库引擎；来料通知**彻底删除**（模块/handler/页面/枚举/表/SQL 16 分支全清，历史 source_type=16 台账已清）。
> 关联：[`wms-work-center-hub.md`](wms-work-center-hub.md) §10（work-center 收货 drawer）

## 背景

来料通知（ArrivalNotice）原承载三职责：收货登记 + 质检卡点（QMS）+ 应付立账/PO回写枢纽（`ArrivalAcceptedHandler`）。工厂不做来料质检 → 质检职责不需要；收货登记 + 立账/回写是业务必须，但不需独立单据承载。取消来料通知，PO 直接收货即入库即立账。

## 核心服务：`PurchaseStockInService`

`abt-core/src/wms/stock_in/PurchaseStockInService::receive_and_stock_in(ctx, db, req)` — 事务内 8 步**同步编排**（替代原 `ArrivalAcceptedHandler` 异步事件，消除窗口期断链）：

1. 幂等 `try_claim(idempotency_key)`
2. 超收校验（`quantity × (1 + over_delivery_allowance_pct/100)`）
3. 逐行 `inventory_transaction.record`（`source_type="purchase_order"` + `source_id=po_id`）
4. 增量累加 PO `received_qty`（`add_received_qty` 行锁，并发部分收货串行化；`order_item_id=0` 时按 product_id 解析，兼容 stock-in/create 多 PO 前端）
5. PO 状态流转（`>=quantity`→Received；`>0`→PartiallyReceived；乐观锁）
6. 立应付（PO 维度 upsert：`source_type=PurchaseOrder` + `source_id=po_id`；多次部分收货 `rewrite_amount_by_source` 重算金额）
7. 成本分录（`CostType::Material`，source=PO）
8. 审计日志

**不自开事务**：接收 `db: PgExecutor`，调用方（abt-web handler）开 `pool.begin()` 后传入 `&mut tx`。

## 财务红线（5 观测点，全在同事务内，半失败全回滚）

| 红线 | 观测点 |
|---|---|
| PO 回写 | `purchase_order_items.received_qty` |
| PO 状态 | `purchase_orders.status` |
| 应付立账 | `ar_ap_ledger` source_type=PurchaseOrder(7)，Credit |
| 成本分录 | `cost_entries` source=PurchaseOrder，Material |
| 库存流水 | `inventory_transactions` source_type="purchase_order" |

**幂等三保险**：① `try_claim`（HTTP 防双击）② 全局唯一索引（migration 072）+ `rewrite_amount_by_source`（重算非累加）③ `add_received_qty` 行锁。

## 数据模型

- `ReceiveAndStockInReq { po_id, rows: Vec<PoStockInRow>, delivery_note, remark, idempotency_key }`
- `PoStockInRow { order_item_id（0=按 product_id 解析）, product_id, received_qty, batch_no, warehouse_id, bin_id }`

## 调用方

- **work-center** `receive_po` action（`po_receive_drawer_body` → `dispatch_action`，单 PO 就地收货）
- **stock-in/create** `handle_purchase_stock_in`（多 PO 批量入口，按 PO 分组逐个调 service）

## 下游 SQL

`fms/ar_ap/repo.rs` 下游 SQL（`product_field_cond`/`rep_cond`/`query_with_party`/`query_details`/`get_detail_row`/`get_detail_items`）按 `source_type=7`（PurchaseOrder）查询 `purchase_order_items`。source_type=16 分支已随 2b 删除。

## 清理记录（2a + 2b，2026-06）

- **2a 代码清理**：删 `arrival_notice` 模块、`arrival_handler`、`wms_arrival_*` 页面/路由/侧边栏、stock-in/create arrival 分支、`recompute_received_qty`、cash_journal ArrivalNotice 分支。
- **2b 彻底删除**：删 `DocumentType::ArrivalNotice=16` + `InspectionSourceType::ArrivalNotice=1` 枚举（+ QMS 映射/repo default/页面标签）、删 AR/AP SQL 全部 source_type=16 分支、新建 migration `075_drop_arrival_notices.sql`（DROP 表+索引）、清历史 source_type=16 台账数据（DELETE 34 笔 + settlements 3）。

来料通知已**彻底移除**：无模块、无表、无枚举、无页面、无前端入口。
