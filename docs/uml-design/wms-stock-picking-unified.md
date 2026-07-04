# WMS 统一库存作业单据模型（stock_picking）设计

> **状态**：设计草案，待评审。评审通过后再进入实现（分阶段，每阶段独立 PR）。
> **关联**：本设计根因治理 `wms-work-center.md` 作业中心跨表 UNION、list/detail 分散、状态机不一致的痛点。
> **参照**：Odoo `stock.picking`（统一表派，业界最主流）、ERPNext `Stock Entry`+`Stock Ledger Entry`（半统一）、OFBiz（多实体，ABT 现状）。

## 1. 背景：当前 5 表分散的痛点

ABT 现状库存作业单据按业务分 5 张表（接近 OFBiz 多实体派）：

| 业务 | 表 | 状态枚举 |
|---|---|---|
| 采购/生产收货 | `purchase_orders` + `work_orders`（按 status + received_qty 推断待收） | PO/WO 各自 |
| 销售发货 | `shipping_requests` | Draft/Confirmed/Picking/Shipped/Cancelled |
| 生产领料 | `material_requisitions` | Draft/Confirmed/Issued/PartiallyIssued/Cancelled |
| 库存调拨 | `inventory_transfers` | Draft/InTransit/Completed/Cancelled |
| 循环盘点 | `cycle_counts` | Draft/Counting/Completed/Adjusted/PendingReview/Cancelled |

痛点：
- **作业中心跨表 UNION 聚合**（`work_center/repo.rs` 5 段 SQL + Arrival 的 PO∪WO UNION ALL），新增业务要改聚合查询
- **状态机不一致**：每域独立状态枚举，作业中心要按域映射 `statuses: &[i16]`
- **list / detail 页分散**：每域一套 list/detail/create 页（收口工作中已暴露，5 域 × 3 页 = 15 页）
- **EventHandler / 回写逻辑按域割裂**（ArrivalAcceptedHandler / shipping 回写 SO / requisition 回写工单成本各自一套）
- **未来扩展（退货/委外/生产补料）每加一类就要加一张表 + 全套 service/list/detail**

## 2. 三家 ERP 对照

| ERP | 流派 | 作业单据 | 区分业务 | 底层流水 |
|---|---|---|---|---|
| **Odoo** | 🥇 统一表 | `stock.picking` 一张表 | `picking_type_id.code`：incoming/outgoing/internal | `stock.move`（行级实际移动）→ `stock.quant` |
| **ERPNext** | 🥈 半统一 | `Stock Entry`（内部移动+生产）；`Purchase Receipt`/`Delivery Note`（购销独立） | `Stock Entry.purpose`：Material Issue/Receipt/Transfer/Manufacture... | `Stock Ledger Entry`（全统一 append-only） |
| **OFBiz** | 🥉 多实体 | `Shipment`+`ShipmentReceipt`+`InventoryTransfer`+`ItemIssuance` | `Shipment.shipmentTypeId`（仅出货内部） | 各实体字段自带 |
| **ABT 现状** | OFBiz 派 | 5 张独立表 | 每表一个 enum | `inventory_transactions`（已统一 append-only）✅ |

**关键**：ABT 底层流水（`inventory_transactions`）**已经是统一的**（TransactionType 枚举 + source 关联），只差上层作业单据统一。这是 Odoo/ERPNext 都验证过的双层结构。

**盘点**（`cycle_counts`）三家都独立表——盘点是"核对"非"移动"，**不纳入 stock_picking**（本设计不动盘点）。

## 3. 目标与原则

**目标**：把 4 张作业单据表（收货/发货/领料/调拨）合并为 1 张 `stock_pickings`（+ 明细 `stock_picking_items`），按 `picking_type` 区分业务，统一状态机。盘点保持独立。

**原则**：
1. **只统一上层作业单据**；下层 `inventory_transactions` 流水保持不变（done 时照常写流水）
2. **picking_type 区分业务语义**（方向 + 来源），不消灭业务差异，而是用类型 + 扩展字段承载
3. **统一状态机 + 行级部分量**：状态机只表达"作业生命周期"，业务特有的"部分发料/在途/拣货中"用行级 `qty_done` + 关联子单（pick_list）表达
4. **向后兼容**：迁移期旧表保留（双写或视图），分阶段切流，每阶段可独立回滚
5. **单据 ≠ 流水**：`stock_pickings` 是可改的作业单据（draft→done）；`inventory_transactions` 仍是 append-only 的库存账（不变）

## 4. 统一模型设计

### 4.1 `stock_pickings`（单据头）

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | bigserial PK | |
| `doc_number` | varchar unique | 单据号（`PICK-...` 或按 type 前缀 `WH-IN/WH-OUT/TRF/REQ`） |
| `picking_type` | enum | 业务类型（见 4.3） |
| `status` | enum | 统一状态机（见 4.4） |
| `source_type` | varchar | 来源单据类型：`purchase_order` / `work_order` / `sales_order` / `none` |
| `source_id` | bigint nullable | 来源单据 id |
| `partner_id` | bigint nullable | 客户/供应商（发货/收货用） |
| `from_warehouse_id` / `from_zone_id` / `from_bin_id` | bigint nullable | 源库位（发货/调拨/领料的出库侧） |
| `to_warehouse_id` / `to_zone_id` / `to_bin_id` | bigint nullable | 目标库位（收货/调拨的入库侧） |
| `operator_id` | bigint | 操作员 |
| `scheduled_date` | date nullable | 计划日期（到期，驱动紧急度） |
| `done_at` | timestamptz nullable | 完成时间 |
| `pick_list_id` | bigint nullable | 关联拣货单（发货拣货子流程，复用现有 pick_lists） |
| `work_order_id` | bigint nullable | 关联工单（领料/生产入库用） |
| `remark` / `created_at` / `updated_at` / `deleted_at` | | 标准字段 |

> **from/to 库位语义**（Odoo location_id/location_dest_id）：
> - 收货（Incoming）：from = 供应商（虚拟），to = 入库仓/库位
> - 发货（Outgoing）：from = 出库仓/库位，to = 客户（虚拟）
> - 调拨（Transfer）：from = 源仓，to = 目标仓
> - 领料（Issue）：from = 仓，to = 工单/工序（虚拟）

### 4.2 `stock_picking_items`（明细）

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | bigserial PK | |
| `picking_id` | bigint FK → stock_pickings | |
| `product_id` | bigint | |
| `batch_no` | varchar nullable | 批次 |
| `qty_requested` | decimal | 申请/需求量 |
| `qty_done` | decimal | 实际量（行级部分完成：`qty_done < qty_requested`） |
| `from_bin_id` / `to_bin_id` | bigint nullable | 行级库位（拣货/上架） |
| `operation_id` | bigint nullable | 工序（领料用） |
| `source_item_id` | bigint nullable | 来源单据明细行（PO/SO 行） |
| `remark` | varchar nullable | |

> 行级 `qty_done` 表达部分量（替代 `PartiallyIssued` 状态）。拣货明细复用现有 `pick_list` 子单（不并入 item）。

### 4.3 `picking_type` 枚举

```rust
pub enum PickingType {
    IncomingPurchase,    // 采购收货（source = PO）
    IncomingWorkOrder,   // 生产入库（source = WO）
    OutgoingSales,       // 销售发货（source = SO）
    InternalTransfer,    // 库存调拨（仓→仓）
    InternalIssue,       // 生产领料（仓→工单/工序）
    // 未来扩展（不加表，仅加枚举值 + service 分支）：
    // IncomingReturn,    // 采购退货
    // OutgoingReturn,    // 销售退货
    // OutsourceIssue,    // 委外发料
}
```

映射作业中心 5 tab：Arrival = IncomingPurchase ∪ IncomingWorkOrder；Outbound = OutgoingSales；Requisition = InternalIssue；Transfer = InternalTransfer；CycleCount 不变。

### 4.4 统一状态机

```rust
pub enum PickingStatus {
    Draft,      // 草稿
    Confirmed,  // 已确认，待执行（进入作业中心待办）
    Done,       // 已完成（写 inventory_transactions 流水）
    Cancelled,  // 已取消
}
```

**业务子流程如何表达**（关键设计点，待评审）：
- **拣货中**（shipping 的 Picking）：`status=Confirmed` + `pick_list_id` 非空 + pick_list 自身状态。作业中心按 `pick_list` 阶段渲染（沿用现有 Outbound 三阶段 Unpicked/Picking/ReadyToShip 逻辑，改读 pick_list）
- **部分发料**（requisition 的 PartiallyIssued）：行级 `qty_done < qty_requested`，单据仍 `Confirmed` 直到全部发完才 `Done`
- **在途**（transfer 的 InTransit）：调拨 dispatch 后 `Confirmed` + 行级 from 扣减，complete 后 `Done` + 行级 to 增加（或用 `in_transit` 布尔）

> **待评审**：是否引入第 5 态 `Assigned`（库存已预留，Odoo 有）？ABT 当前无库存预留机制，**建议初版不加**，未来 ATP 预留时再补。

## 5. 与 `inventory_transactions` 的衔接（下层不变）

`stock_pickings` done 时，service 内部调 `InventoryTransactionService::record()` 写流水（逻辑从现有 4 个 service 搬入）。流水字段映射：

| inventory_transactions | 来源 |
|---|---|
| `transaction_type` | 按 picking_type 映射（PurchaseReceipt/ProductionReceipt/SalesIssue/TransferOut/TransferIn/MaterialIssue） |
| `source_type` / `source_id` | `stock_picking` + picking_id（或保留原 PO/WO/SO 关联） |
| `quantity` | 行级 qty_done |
| `warehouse/zone/bin` | 行级 to（入库）或 from（出库） |

**EventHandler 不变**：ArrivalAcceptedHandler 仍听 ArrivalInspected（或改听 PickingDone + type=IncomingPurchase）；shipping 回写 SO、requisition 回写工单成本——逻辑搬入 PickingService 的 type 分支，事件可统一为 `PickingDone`。

## 6. 统一 PickingService trait

```rust
#[async_trait]
pub trait PickingService: Send + Sync {
    async fn create(&self, ctx, db, req: CreatePickingReq) -> Result<i64>;
    async fn get(&self, ctx, db, id: i64) -> Result<Picking>;
    async fn list(&self, ctx, db, filter: PickingFilter, page: PageParams) -> Result<PaginatedResult<Picking>>;
    async fn list_items(&self, ctx, db, id: i64) -> Result<Vec<PickingItem>>;
    async fn confirm(&self, ctx, db, id: i64) -> Result<()>;
    async fn cancel(&self, ctx, db, id: i64) -> Result<()>;
    /// 执行（按 type 分发：发货=ship/拣货、领料=issue、调拨=dispatch、收货=receive）
    /// done 时事务内写 inventory_transactions + 回写来源单据 + 发 PickingDone 事件
    async fn done(&self, ctx, db, id: i64, items: Vec<DoneItemReq>) -> Result<()>;
}
```

`done()` 内部按 `picking_type` 分发具体业务逻辑（库存扣增、成本、应收应付立账、来源回写）——这些逻辑从现有 4 个 service 搬入。

> **领料落地工单关联 + 行级 bin/batch**（`create_manual`）：`CreateManualReq` 含 `work_order_id: Option<i64>`（Some 时 picking.source_id / work_order_id 关联工单，替代黑盒 `create_for_work_order` 全量展开，前端展示 BOM + 用户调量后提交具体行）；`CreateManualItemReq` 含 `bin_id` / `batch_no`（行级库位 + 批次，落 `from_bin_id` / `batch_no`）。前端「选工单→加载 BOM 行」经 `list_wo_requisition_preview`（BOM `leaf_nodes` × `planned_qty` − `sum_issued_qty_by_work_order` 已领量）算待领差额 + `query_available_batch` 可用量 → 渲染整组行 fragment（`HX-Trigger-After-Settle: woItemsLoaded`）；统一领料仓取首行 `warehouse_id`（对齐 `receive_purchase` 头仓范式）。

## 7. 迁移策略（数据 + 类型 + 状态映射）

**类型映射**：
| 旧表 | → stock_pickings.picking_type |
|---|---|
| PO 收货 | IncomingPurchase |
| WO 入库 | IncomingWorkOrder |
| shipping_requests | OutgoingSales |
| material_requisitions | InternalIssue |
| inventory_transfers | InternalTransfer |

**状态映射**：
| 旧状态 | → PickingStatus |
|---|---|
| Draft / Picking（shipping 拣货中，转 Confirmed + pick_list_id） | Confirmed（或 Draft） |
| Confirmed / PartiallyIssued（行级部分） / InTransit | Confirmed |
| Shipped / Issued / Completed / Received | Done |
| Cancelled | Cancelled |

**迁移脚本**：每张旧表一条 `INSERT INTO stock_pickings SELECT ...` + `INSERT INTO stock_picking_items SELECT ...`，事务内 + 校验 row count。旧表迁移期保留（只读视图或双写），切流后再删。

## 8. 影响面

| 层 | 影响 |
|---|---|
| **数据库** | 新增 `stock_pickings` / `stock_picking_items` + migration；旧 4 表迁移期保留 |
| **abt-core** | 新增 `wms/picking/` 模块（model/repo/service/implt）；旧 4 service 逻辑搬入；EventHandler 改听 PickingDone（或过渡期双发） |
| **作业中心** | `work_center/repo.rs` 简化为单表查询（`WHERE picking_type IN (...) AND status = Confirmed`），删 UNION；domain 仍 5 tab（按 picking_type 分组） |
| **list/detail/create 页** | 统一为 1 套 picking list/detail/create（按 type 渲染差异字段），取代 4 套（盘点独立） |
| **前端组件** | product_picker / bin_picker 复用；shipping_request_picker → picking_picker |
| **跨模块** | 销售（SO→发货）、采购（PO→收货）、MES（WO→领料/入库）的 service 调用从旧 service 改 PickingService |

## 9. 实施路线（分阶段，每阶段独立 PR + 可回滚）

1. **建表 + 模型 + 空 PickingService**（不接业务，单测）—— 风险最低
2. **迁移领料**（InternalIssue）：requisition 表双写 → 作业中心/前端切 picking → 验证 → 旧表只读
3. **迁移调拨**（InternalTransfer）
4. **迁移发货**（OutgoingSales，含拣货子流程）
5. **迁移收货**（IncomingPurchase + IncomingWorkOrder，含 EventHandler）
6. **删旧 4 表 + 旧 service**（确认无引用）
7. **盘点**（可选：是否也统一为 PickingType.InventoryCount？建议不动，保留独立）

每阶段：migration + service + 前端 + 作业中心切流 + clippy + e2e 验证。一个阶段出问题可回滚到上阶段（旧表还在）。

## 10. 待评审点 + 风险

**待评审**：
1. **状态机粒度**：4 态（Draft/Confirmed/Done/Cancelled）够吗？还是要 `Assigned`（预留）？业务子流程（拣货/在途/部分量）用行级 + 关联子单表达是否接受？
2. **拣货子流程**：保留独立 `pick_lists` 表 + `pick_list_id` 外键（方案 A），还是把拣货明细并入 `stock_picking_items`（方案 B）？方案 A 改动小、保留现有拣货逻辑；方案 B 更纯粹但要重写拣货
3. **来源单据字段**：`source_type/source_id` 保留泛化（PO/WO/SO/none），还是每类一个外键（po_id/wo_id/so_id）？泛化更灵活，外键更严格
4. **doc_number 规则**：统一 `PICK-` 前缀，还是按 type 前缀（WH-IN/WH-OUT/TRF/REQ）便于人工识别？后者更友好
5. **事件统一**：现有 ArrivalInspected / ShippingShipped / RequisitionIssued 等事件是统一为 `PickingDone`（按 type 分发），还是保留各域事件（PickingService 内部发各域事件）？后者改动小
6. **盘点归属**：确认盘点保持独立（cycle_counts 不变）

**风险**：
- 数据迁移（4 表 → 1 表）涉及历史数据，需在备份 + 低峰期跑，校验 row count + 金额
- 跨模块调用面广（销售/采购/MES/委外/财务），切换期需保证 EventHandler 不丢
- 作业中心 UNION → 单表的切换要同步（迁移期数据双轨，作业中心读哪边？建议迁移完一个域就切一个域）

---

> **下一步**：用户评审本设计（尤其 §10 待评审点 1-6 拍板）。通过后进入阶段 1（建表 + 空 service），按 §9 路线逐阶段实施。**当前 `feat/wms-work-center-menu-collapse` 的 5 个收口 commit 作为短期可用基线保留**（统一表实现后，收口工作的"作业中心全部视图"部分会被单表查询简化替代，但侧边栏收口 + 详情 drawer 等仍可复用）。
