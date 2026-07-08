# 销售发货职责归属重构：shipping_request 从 sales 迁至 wms

> 关联 Issue：[#93](https://github.com/swloki/abt2/issues/93)　|　消解：[#36](https://github.com/swloki/abt2/issues/93)（出库管理页缺审核入口）
> 状态：**设计骨架，待评审**（接口与模型先行，评审确认后再实施代码）
> 现行实现：`abt-core/src/sales/shipping_request/`；前端 `/admin/shipping/*` + `/admin/wms/stock-out/*`

## 1. 背景与目标

### 1.1 问题
销售发货当前存在**三套割裂的执行路径**，职责归属错位（详见 #93 诊断表）：

1. `/admin/shipping` + `sales/shipping_request` —— 完整单据状态机（Draft→Confirmed→Picking→Shipped），含 QMS 卡控 / 库存预留 / COGS / AR 立账，但住在 `sales/`，`ship()` 却执行仓库（扣库存）+ 财务（COGS / AR 台账）的职责。
2. `/admin/wms/stock-out` + `InventoryTransactionService.record()` —— 直接出库旁路，无单据流转 / 审核 / QMS / AR 立账，与上者数据割裂（#36 根因）。
3. `sales_order` 已有需求池 `DemandService`，但发货**未走需求池**，是 order→shipping 直连，是整条链唯一没解耦的断层。

### 1.2 目标（OFBiz 式职责划分）
> 三家 ERP（ERPNext / Odoo / OFBiz）无一例外把发货执行归仓库模块。ABT 的「预留 + 出库事务」两步架构本就是 OFBiz `reserve→issue` 翻版，**能力已达标，只差归属正确**。

- **销售**：仅负责订单（下单 / 确认 / 取消）与发货需求下发，只读查看发货状态，**不得扣库存 / 立账**。
- **仓库**：负责出库执行单（审核 / 拣货 / 出库 / 扣库存），操作主体归仓库岗。
- **财务**：经领域事件驱动立 AR 台账 / COGS，**业务模块不直访 fms 表**。

## 2. 现状代码边界（迁移前）

`shipping_request/implt.rs` 当前依赖（L1-35）：

| 依赖 | 性质 | 迁移后处置 |
|---|---|---|
| `sales::sales_order::repo::{SalesOrderRepo, SalesOrderItemRepo}` | 同模块 repo 直访（合规） | **跨域 → 改走 `SalesOrderService` trait** |
| `fms::ar_ap::repo::ArApLedgerRepo`（ship 直插 ar_ap_ledger L626） | **跨域 repo 直访（违规）** | **改 `ShipmentShipped` 事件 → fms handler** |
| `qms::inspection_result::service::InspectionResultService` | service trait（合规） | 保留 |
| `wms::inventory_transaction::service::InventoryTransactionService` | service trait（合规） | 保留（迁入后变同域） |
| `shared::*`（reservation / cost_entry / state_machine / event_bus / document） | 共享服务（合规） | 保留 |

`ship()` 单方法 6 步写事务（改 shipped_qty → 释放预留 → 扣库存 → COGS → AR 立账 → 状态机），已是有事故史的过重入口（见 `fms-ar-ap.md` SO-2026-06-000170）。

## 3. 目标架构（迁移后）

```
sales::sales_order            wms::outbound                  fms (事件驱动)
─────────────────            ──────────────                 ──────────────
下单 / 确认 / 取消     ──下发发货需求──►  审核确认(confirm)
需求池 DemandService                       拣货(pick)
只读 delivery_status  ◄──回写 shipped_qty──  出库(ship)
                                              ├─ 扣库存(inventory_transaction)
                                              ├─ 释放预留(reservation)
                                              └─ 发 ShipmentShipped 事件 ──► 立 AR 台账 + COGS
```

## 4. 接口设计（Service trait）★

> 跨模块调用**只允许 Service trait + Model**，禁止直访 Repository（CLAUDE.md 模块边界铁律）。

### 4.1 `wms::outbound`（迁入）— `OutboundShipmentService`

```rust
#[async_trait]
pub trait OutboundShipmentService: Send + Sync {
    // —— 仓库岗操作（操作主体：仓库）——
    async fn create_from_order(&self, ctx, db, req: CreateFromOrderReq) -> Result<i64>; // sales 下发入口
    async fn save_draft(&self, ctx, db, req: CreateDraftReq) -> Result<i64>;
    async fn update_draft(&self, ctx, db, id: i64, req: UpdateDraftReq) -> Result<()>;
    async fn find_by_id(&self, ctx, db, id: i64) -> Result<OutboundShipment>;
    async fn confirm(&self, ctx, db, id: i64) -> Result<()>;  // 仓库审核（OQC 卡控保留）
    async fn pick(&self, ctx, db, req: PickReq) -> Result<()>; // 仓库拣货（二期落独立拣货单）
    async fn ship(&self, ctx, db, id: i64) -> Result<()>;      // 仓库出库
    async fn cancel(&self, ctx, db, id: i64) -> Result<()>;
    async fn list(&self, ctx, db, filter: OutboundQuery, page: PageParams)
        -> Result<PaginatedResult<OutboundShipment>>;
    async fn list_items(&self, ctx, db, id: i64) -> Result<Vec<OutboundShipmentItem>>;
}
```

**`ship()` 新边界（只做仓库 + 事件，不碰 fms）**：
1. 校验 Picking
2. 逐明细扣实物库存：`inventory_transaction.record(SalesShipment, 负向)`
3. 释放预留：`inventory_reservation.fulfill_by_source_line`
4. 回写订单：`SalesOrderService::record_shipment(order_id, lines)`（见 4.2，**跨域走 trait**，不再直访 sales repo）
5. 状态机 → Shipped + 审计
6. **发 `ShipmentShipped` 事件**（替原直插 ar_ap_ledger / cost_entry）
7. 整体 `pool.begin() + commit()` 事务包裹（铁律，见 `fms-ar-ap.md` 事务边界）

### 4.2 `sales::sales_order` — 瘦身 + 新增回写接口

**保留**：`create / create_from_quotation / update / confirm / complete / cancel / list / list_items / list_items_by_order_ids / cancel_line / list_fulfillment_plan / recalc_header_status / DemandService` 全套。

**移除**：销售单上一切扣库存 / 发货执行入口（发货动作全部移交 wms）。

**新增（供 wms 回写 + 只读状态）**：
```rust
/// wms 出库后回写订单行已发数量 + 重算头状态（事务内，与扣库存原子）
async fn record_shipment(
    &self, ctx, db, order_id: i64, lines: &[ShipmentLineQty],
) -> Result<SalesOrderStatus>;

/// 销售订单详情页只读发货状态（对齐 Odoo delivery_status）
async fn delivery_status(&self, ctx, db, order_id: i64) -> Result<DeliveryStatus>;
```

### 4.3 `fms` — 新增 `ShipmentShippedHandler`（事件驱动立账）

对称已有 `SalesReturnReceivedHandler`（#86）、`ArrivalAcceptedHandler`：
```rust
// 消费 ShipmentShipped 事件 → 业财一体立账（幂等）
async fn handle(ctx, db, event: ShipmentShipped) {
    // 1. 经 SalesOrderService.list_items 取 unit_price（跨域走 trait）
    // 2. 幂等检查 ar_ap_ledger (source_type=OutboundShipment, source_id)
    // 3. insert AR Debit 台账（金额=Σ shipped_qty × unit_price，按客户 payment_terms 推到期日）
    // 4. COGS 经 shared.cost_entry.create_entries
}
```

### 4.4 跨域调用矩阵

| 调用方 | 被调方 | 方式 | 用途 |
|---|---|---|---|
| `sales.sales_order` | `wms.outbound` | service trait | 下发发货需求、查 `delivery_status` |
| `wms.outbound` | `sales.sales_order` | service trait | 读订单行、`record_shipment` 回写 |
| `wms.outbound` | `qms.inspection` | service trait | confirm 时 OQC 卡控 |
| `wms.outbound` | `shared.*` | factory | reservation / inventory_transaction / state_machine / event_bus / document |
| `fms.handler` | `sales.sales_order` | service trait | `ShipmentShipped` 后取 unit_price 立账 |
| `wms.outbound` → `fms` | — | **禁止直访** | 仅经 `ShipmentShipped` 事件解耦 |

## 5. 模型设计

### 5.1 实体（迁移后命名）
- `OutboundShipment`（原 `ShippingRequest`）：`id / doc_number / order_id / customer_id / status / shipping_address / carrier / tracking_number / operator_id …`
- `OutboundShipmentItem`（原 `ShippingRequestItem`）：`id / outbound_id / order_item_id / product_id / warehouse_id / requested_qty / shipped_qty …`
- **状态机不变**：`Draft(1) → Confirmed(2) → Picking(3) → Shipped(4)` / `Cancelled(5)`

### 5.2 物理表迁移策略（推荐最小风险）
- **保留物理表 `shipping_requests` / `shipping_request_items` 不重命名**，仅改代码归属与类型名映射 → 避免历史数据 / 单据号 / 外键 / ar_ap_ledger.source_id 全链路改动。
- 代码层类型重命名 `ShippingRequest → OutboundShipment`（含 trait / model / repo）。
- `DocumentType::ShippingRequest` 枚举**保留原值**（ar_ap_ledger / inventory_transaction / document_links 已大量引用），仅注释语义更新为「出库执行单」。
- 评审决议点：是否接受「表名保留 + 代码层正名」折中，或坚持物理重命名（高成本）。

### 5.3 二期：独立拣货单 `PickList`（参照 ERPNext）

当前 `pick()` 仅空状态切换（Confirmed → Picking）。二期落独立拣货单，让拣货可追溯、可部分拣、可记录库位。

**数据模型**（migration `071`，需手动 `psql -f`）：
- `pick_lists`：`id` / `doc_number`（`PK-` 前缀，`DocumentType::PickList`）/ `outbound_id` FK→`shipping_requests` / `status` / `picker_id` / `picked_at` / `operator_id` / timestamps
- `pick_list_items`：`id` / `pick_list_id` / `line_no` / `outbound_item_id` / `product_id` / `warehouse_id` / `bin_id`(可选) / `requested_qty` / `picked_qty`
- `DocumentType::PickList = 46`（现有最大 45）
- 状态机 `PickListStatus`：`Draft(1)` → `Picked(2)` / `Cancelled(3)`

**`PickListService` trait**（`abt-core/src/wms/pick_list/`）：
- `generate_from_outbound(ctx, db, outbound_id)` — `pick()` 调用，从 outbound 明细生成 pick_list + items（MVP：`picked_qty = requested_qty` 自动满拣，前端后续支持人工调整）
- `complete_pick(ctx, db, id)` — Draft → Picked
- `find_by_outbound(ctx, db, outbound_id)` / `list` / `list_items` / `cancel`

**流转**：outbound `confirm` → `pick()` 调 `generate`（生成 pick_list Draft，outbound → Picking）→ 仓库前端录入 `picked_qty`/`bin` → `complete_pick`（Picked）→ `ship()`。MVP 不强依赖前端（自动满拣），前端拣货录入页为 Phase 3 收尾。

**实施任务**（Phase 3，建议 #94 合并后独立 PR）：
1. migration `071`（pick_lists + pick_list_items + state_machine 配置）+ `DocumentType::PickList` + `PickListStatus`
2. `wms/pick_list/` 模块（model/repo/service/implt/mod）
3. outbound `pick()` 集成（调 generate）/ `ship()` 集成（校验 pick_list Picked）
4. 前端拣货录入页（`/admin/wms/shipping/{id}/pick`，需 Open Design 原型）
5. cargo clippy + commit

## 6. stock-out 旁路合并

- 废弃 `/admin/wms/stock-out`（`wms_stock_out_*.rs` / `routes/wms_stock_out.rs`）的直接出库路径。
- 该路径调 `InventoryTransactionService.record()` 裸出库、无单据 / 无审核 / 无立账，统一并入 `OutboundShipment` 单据流。
- 评审决议点：是否保留「杂项出库 / 盘点出库」等非销售出库的快速入口（若保留，明确界定其不经 AR 立账的边界）。

## 7. 前端页面入口调整

| 现状 | 迁移后 |
|---|---|
| `/admin/shipping/*`（sales 模块下） | `/admin/wms/shipping/*`（wms 出库管理下，仓库岗操作 confirm/pick/ship） |
| `/admin/wms/stock-out/*`（旁路） | **合并废弃**，跳转至出库单 |
| 销售订单详情页的发货 / 扣库存入口 | 移除，改为只读「发货状态」+ 「下发发货需求」按钮 |

侧边栏「出库管理」归入 WMS，#36 的「缺审核入口」由出库单的 confirm 流程自然提供。

## 8. 实施阶段

- **Phase 1（后端边界重构）**：`shipping_request` 迁至 `wms::outbound`；`ship()` 去掉 fms 直访、改 `ShipmentShipped` 事件；新增 `SalesOrderService::record_shipment / delivery_status`；新增 `fms::ShipmentShippedHandler`。`cargo clippy` + 既有 shipping 测试全绿。
- **Phase 2（前端入口迁移）**：路由 `/admin/shipping → /admin/wms/shipping`；合并废弃 `/admin/wms/stock-out`；销售订单详情页去扣库存入口、加只读发货状态。
- **Phase 3（二期）**：独立 `PickList` 拣货单实体。

每阶段独立 PR，远程 `weichen`，走 feature 分支（`feat/wms-outbound-restructure-*`）。

## 9. 风险与回滚

- **事务边界**：`ship()` 必须 `pool.begin() + commit()` 包裹全步骤，禁止 `RequestContext.conn`（autocommit）——否则中途失败留脏数据（SO-2026-06-000170 事故）。
- **事件立账最终一致**：AR 台账从「同步直插」改「异步事件」后，存在短窗口台账未立；需保证 handler 幂等（`ar_ap_ledger` partial unique index 已就绪）+ 失败重试 / 死信可见。
- **回写并发**：`record_shipment` 并发更新 `shipped_qty` 需行级锁或乐观锁，避免部分发货覆盖。
- **回滚**：Phase 1 若出问题，因物理表 / DocumentType 不变，可回退代码归属而不动数据。

## 10. 销售一键「申请发货」+ ShippingRequested 状态（本期实现）

**背景**：原 `create_from_order` 要求销售在 `/admin/wms/shipping/create` 为每行选仓库（销售不管货物存哪），发货单创建为 Draft（仓库 confirm 前不进 work-center 待发货），订单缺「已申请发货」语义。

**实现**（migration `074` + 代码）：
- **新状态 `SalesOrderStatus::ShippingRequested = 8`**：销售一键申请后推进到此态。`recalc_header_status` 叠加判定——`calc_header_status` 出 Confirmed/ReadyToShip 时，若该订单有活跃发货单（Confirmed/Picking）且未全 Shipped → ShippingRequested（业务意图优先于「库存已补足」中间态）。
- **`ShippingRequestService::request_from_order(order_id, items: Vec<RequestShippingItemReq>, shipping_requirements: String)`**：订单详情页弹窗调用，各行 `warehouse_id=None`（销售不选仓库），发货单跳过 Draft → 直接 Confirmed（进 work-center 待发货队列），回写订单 ShippingRequested。允许 ShippingRequested 状态重复申请（追加数量）。`shipping_requirements` 为销售填写的发货要求（Issue #218），落到 `stock_pickings.shipping_requirements`。
- **`stock_pickings.shipping_requirements TEXT NOT NULL DEFAULT ''`（migration 094，Issue #218）**：销售在申请发货弹窗填写的发货要求（指定快递 / 防震包装等，textarea 上限 200 字），供仓库/物流参考；发货详情页与出库单打印变量（`{{发货要求}}`）展示。其他作业类型（领料/调拨/入库）该列恒为 `''`。
- **`shipping_request_items.warehouse_id` / `pick_list_items.warehouse_id` 改可空**（migration 074 DROP NOT NULL）：销售申请不带仓库，仓库拣货时手选。
- **拣货 drawer 加仓库+库位 select**（`wms_work_center.rs`）：仓库在拣货时决定从哪个仓库/库位出；`PickItemInput` 加 `warehouse_id`，`update_picked` 回填 `pick_list_items.warehouse_id/bin_id`。
- **`ship()` 扣库存改从 `PickListItem` 取 warehouse_id/bin_id**（不再用发货明细的 None）：`outbound_item_id → (warehouse_id, bin_id)` 映射，拣货未录仓库时报「拣货未录入仓库，无法出库」兜底。
- **可行性支撑**：库存预留本就跨仓库（`ReserveRequest.warehouse_id=None`，product 维度 ATP），销售不指定仓库在模型上成立。

**入口**：销售订单详情页「申请发货」按钮（`/admin/orders/{id}/request-ship`，GET 返回 modal + POST 提交）；保留 `/admin/wms/shipping/create` 独立创建页用于复杂场景（指定仓库 / 多订单合并）。work-center 待发货卡片新增「处理」跳转到发货详情。

**验证**：`abt-web/tests/ar_ap_handler_e2e.rs::k5_one_click_request_ship_to_work_center`（申请 → 订单 ShippingRequested(8) + 发货单 Confirmed(2) 跳过 Draft + 明细 warehouse NULL）。
