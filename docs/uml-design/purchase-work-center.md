# 采购作业中心（WorkCenter）设计

> 关联：采购 SRM 各子域（需求池 / 报价 / 订单 / 对账 / 付款 / 退货 / 请购）已齐备，缺一个「采购员一进系统就知道先做什么」的聚合作业页。
> 参照：`wms-work-center.md`、`mes_work_center`（组件化单端点模式）。
> 现状：`purchase_dashboard`（`/admin/purchase`）是纯看板——stat card 只计数、待办不可操作、「最近活动」硬编码假数据。

## 1. 定位

采购是**计划 + 执行闭环最长**的业务域（需求 → 询价 → 下单 → 收货 → 对账 → 付款，逆向退货）。采购岗需要一个**作业中心**，把分散在各列表页的「待处理」状态聚合到一屏，就地审批 / 确认 / 发货，不跳详情页。

范式与 MES / WMS work_center 一致：**组件化单端点**（每个 card 一个 GET 端点 + `hx-select` 局部刷新）+ **HX-Trigger 事件联动**（写操作广播，相关 card 自刷新）+ **drawer 就地操作**。

## 2. PurchaseWorkCenterService 接口

```rust
#[async_trait]
pub trait PurchaseWorkCenterService: Send + Sync {
    /// 聚合各业务分组待办计数（首页锚点条 + 各 card 用）
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<PurchaseWorkCenterSummary>;

    /// 采购订单行展开聚合（订单 card row-detail）：收货进度 + 来源链 + 应付台账。
    async fn get_po_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, order_id: i64) -> Result<PoHubSummary>;

    /// 三单匹配校验查询（付款 drawer ci-row）：复用 payment::approve 校验口径（容差 ±0.5%）。
    async fn check_three_way_match(&self, ctx: &ServiceContext, db: PgExecutor<'_>, payment_id: i64) -> Result<ThreeWayMatchSummary>;

    /// 对账付款行展开聚合（draft 对账单 / payment 待审批付款 两类分发）。
    async fn get_settlement_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, recon_type: SettlementReconType, ref_id: i64) -> Result<SettlementHubSummary>;

    /// 采购退货行展开聚合（退货 card row-detail）：来源 PO + 退货明细 + 结算状态。
    async fn get_return_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, return_id: i64) -> Result<ReturnHubSummary>;
}
```

**设计原则**（同 WMS / MES）：WorkCenterService 是**只读聚合层**——既做聚合计数（`summary`），也做行展开详情聚合（`*_hub_summary` / `check_three_way_match`）。各 card 列表复用现有 service 的 `list`（按状态过滤）；聚合方法**经 trait 跨域调** purchase 各子域 + `ArApService`（fms）+ `DocumentLinkService`（shared）+ `SupplierService`（master_data），**不直访任何 repo**。细项查询失败 best-effort 容错（返回默认 + `tracing::warn!`），不连累整行/整页（同 `summary` 哲学）。

## 3. PurchaseWorkCenterSummary model

```rust
pub struct PurchaseWorkCenterSummary {
    pub pending_demand: u64,            // 待处理外购需求（物料维度）
    pub pending_misc: u64,              // 待审批零星请购（Draft）
    pub po_pending_approval: u64,       // PO 待审批（PendingApproval）
    pub po_pending_receive: u64,        // PO 待收货（Confirmed）
    pub po_partial: u64,                // PO 部分收货（PartiallyReceived）
    pub recon_draft: u64,               // 草稿对账单（Draft）
    pub payment_pending_approval: u64,  // 付款申请待审批（Draft）
    pub return_pending_ship: u64,       // 采购退货待发货（Confirmed）
    pub return_shipped: u64,            // 采购退货已发出（Shipped）
    pub overdue_count: u64,             // 逾期：待收货订单期望交期早于今日
    pub soon_count: u64,                // 临期：待收货订单期望交期在 7 天内
}
```

`total()` = 前 9 项之和（不含 overdue/soon，避免与待收货计数重复）。

## 3.5 行展开聚合模型（row-detail）

每个 card 行内展开（chevron `hx-get row-detail` + `hx-swap="afterend"`）调对应 hub_summary，返回 detail-grid（4 列）。模型定义见 `abt-core/src/purchase/work_center/model.rs`：

| 模型 | 方法 | 关键字段 |
|---|---|---|
| `PoHubSummary` | `get_po_hub_summary(order_id)` | `order` + `progress{ordered/received/returned/inspected_qty, received_pct, item_count}` + `source_chain{sales_order_docs[]}` + `ap_summary{ap_amount, paid_amount}` |
| `ThreeWayMatchSummary` | `check_three_way_match(payment_id)` | `po_matched / receipt_matched / invoice_matched / can_pay / differences[]` |
| `SettlementHubSummary` | `get_settlement_hub_summary(recon_type, ref_id)` | `draft_recon{total/confirmed/difference, pending_returns_*, ap_outstanding}`（DraftRecon）或 `pending_payment{amount/method/invoice, source_recon_doc, three_way_match, ap_outstanding}`（PendingPayment） |
| `ReturnHubSummary` | `get_return_hub_summary(return_id)` | `return_order` + `source_po_doc/status` + `item_count/total_qty` + `settlement_hint` |

`SettlementReconType::parse("draft" | "payment")` 从路径参数解析（settlement card 当前 2 tab：草稿对账单 / 待审批付款）。

## 3.6 跨域依赖（聚合方法消费的共享 / 他域 Service）

| 依赖 | 用途 | 调用方法 |
|---|---|---|
| `ArApService`（fms） | PO 立账/已付 + 供应商 AP 余额 | `list_ledger(doc_no=po.doc_number, party_id)` post-filter `source_id`；`get_party_balance(Supplier, id).total_ap` |
| `DocumentLinkService`（shared） | PO → SO 来源链 | `find_linked(PurchaseOrder, order_id)` 过滤 `target_type=SalesOrder` |
| `SupplierService`（master_data） | 供应商名 | `get(supplier_id).name` |
| `PurchaseOrderService` / `PurchaseReconciliationService` / `PaymentRequestService` / `PurchaseReturnService` | 各子域 get / list_items | 复用既有 trait |

> **AP 反查契约（R1）**：`ArApLedgerFilter` 无 `source_id` 字段，PO 立账经 `doc_no=po.doc_number + party_id` 过滤后 post-filter `source_type=PurchaseOrder && source_id=po.id` 精确匹配。详见 `fms-ar-ap.md`。
> **三单匹配（A.3）**：提炼自 `PaymentRequestServiceImpl::approve` 的私有校验（`within_tolerance` 容差 ±0.5%），不改动 approve 流程；「未提供」（无对账单 / 无发票）按 approve 现有逻辑放行。

## 4. 各 card「待办」状态边界

| Card | tab / 数据源 | pending 状态（枚举值） | Service |
|---|---|---|---|
| ① 采购需求 | 物料汇总 / 请购明细 | `demand_status=1`（Pending） | `PurchaseDemandService::list_material_aggregated` / `list_pending_demands` |
| ① 采购需求 | 请购（misc） | `MiscRequestStatus::Draft` | `MiscellaneousRequestService::list` |
| ② 采购订单 | 待审批 | `PurchaseOrderStatus::PendingApproval` | `PurchaseOrderService::list` |
| ② 采购订单 | 待收货 | `PurchaseOrderStatus::Confirmed` | 同上 |
| ② 采购订单 | 部分收货 | `PurchaseOrderStatus::PartiallyReceived` | 同上 |
| ③ 对账付款 | 草稿对账单 | `PurchaseReconStatus::Draft` | `PurchaseReconciliationService::list` |
| ③ 对账付款 | 待审批付款 | `PaymentStatus::Draft` | `PaymentRequestService::list` |
| ④ 采购退货 | 待发货 | `PurchaseReturnStatus::Confirmed` | `PurchaseReturnService::list` |
| ④ 采购退货 | 已发出 | `PurchaseReturnStatus::Shipped` | 同上 |

## 5. 实现策略

`PurchaseWorkCenterServiceImpl::summary`（`abt-core/src/purchase/work_center/implt.rs`）：

- 按需工厂获取各 service（`new_purchase_order_service(self.pool.clone())` 等，struct 只持 `PgPool`）
- 每项计数调对应 `list(status=..., PageParams::new(1,1))` 取 `total`，经 `cnt()` helper **容错**：单项查询失败 `tracing::warn!` 后记 0，不连累整页（同 MES）
- **逾期 / 临期**：扫描待收货（Confirmed + PartiallyReceived）订单首页 500 条，按 `expected_delivery_date` 判定（`< today` 逾期，`<= today+7` 临期），近似统计

## 6. 前端（`abt-web`）

**路由**（`/admin/purchase/work-center`）：

| 路径 | 方法 | 说明 |
|---|---|---|
| `/admin/purchase/work-center` | GET | 主页（detail-header + 锚点条 + 4 card shell + 2 drawer overlay） |
| `/admin/purchase/work-center/demand` | GET | ① 需求 card（tab + 搜索 + `hx-select=#pc-demand-card`） |
| `/admin/purchase/work-center/orders` | GET | ② 订单 card |
| `/admin/purchase/work-center/settlement` | GET | ③ 对账付款 card |
| `/admin/purchase/work-center/returns` | GET | ④ 退货 card |
| `/orders/{id}/approve-drawer` | GET | 订单审批 drawer body |
| `/payments/{id}/approve-drawer` | GET | 付款审批 drawer body |
| `/orders/{id}/approve` | POST | 审批通过 → `HX-Trigger: poChanged` |
| `/orders/{id}/reject` | POST | 驳回 → `poChanged` |
| `/reconciliations/{id}/confirm` | POST | 对账确认 → `reconChanged` |
| `/payments/{id}/approve` | POST | 付款审批 → `reconChanged` |
| `/orders/{id}/row-detail` | GET | 订单行展开 detail-grid（调 `get_po_hub_summary`） |
| `/settlement/{recon_type}/{ref_id}/row-detail` | GET | 对账付款行展开（draft / payment 两套 grid） |
| `/returns/{id}/row-detail` | GET | 退货行展开（调 `get_return_hub_summary`） |
| `/demand-rows?product_id=` | GET | 需求物料行懒加载明细（调 `list_pending_demands`） |

**HTMX 契约**：
- card shell（grp 折叠）：`grp` + `grp-head`（图标 + 标题 + meta + chevron）`_="on click toggle .open on closest .grp"`；`grp-body` 占位 div `hx-trigger="load, poChanged from:body, reconChanged from:body, returnChanged from:body"`（懒加载 + 写操作后自刷新）
- card 内 tab/搜索：`hx-target="#pc-xxx-card" hx-select="#pc-xxx-card" hx-swap="outerHTML" hx-push-url="true"`
- 行展开 chevron：`hx-get=...row-detail hx-target="this" hx-swap="afterend" _="on click toggle .open on closest <tr/>"`（依赖 `uno.config.ts` preflight 的 `tr.open .expand-btn svg{rotate(90deg)}` + `.grp.open` 折叠 CSS）
- 需求物料行：`hx-get=...demand-rows?product_id= hx-trigger="click once" _="on click toggle .expanded on #pc-demand-toggle-{pid}"`
- 写操作：事务包裹（`state.pool.begin()...tx.commit()`），返回 `([("HX-Trigger", "poChanged")], Html::empty())`

**事件命名**：
- `poChanged` — 订单审批/驳回（影响订单 card + 锚点条）
- `reconChanged` — 对账确认/付款审批（影响对账付款 card）
- `returnChanged` — 预留（退货发货 Phase 2 就地化时启用）

**drawer**：复用 `render_drawer_overlay`（overlay + `open:` 变体 + Hyperscript 关闭），body 由 `hx-get` 填充。

## 7. Phase 划分

- **Phase 1**：4 只读 card + 锚点条 + 订单审批/付款审批 drawer + 审批/驳回/对账确认/付款审批 4 写操作。
- **Phase 1.5（已实现，原型 `02-work-center.html` 对齐）**：
  - **图标层**：头部 meta chip（日期/本周待办）+ 锚点 chip（4 业务图标）+ 告警 pill（逾期/临期）+ 4 grp-icon + chevron + 物料 mat-ic（紧急度配色）+ ci-row 就绪态 + info-box + hub-link 箭头。
  - **结构层**：grp 分组折叠（`grp-head` + `grp-body`）+ 4 card 行内展开 detail-grid（chevron `hx-swap=afterend`）+ 需求物料卡片化（懒加载 demand-rows）+ 订单收货进度条 + 付款 drawer 三单匹配 ci-row。
  - **abt-core 4 聚合接口**：`get_po_hub_summary` / `check_three_way_match` / `get_settlement_hub_summary` / `get_return_hub_summary`。
- **Phase 2（后续）**：转单 / 登记收货 / 退货发货 / 创建对账单的就地 drawer；退货物流单号字段（`PurchaseReturn` 现无 `tracking_no`，需 migration）；settlement card「待对账入库」tab（当前仅 draft/payment 两 tab）。
