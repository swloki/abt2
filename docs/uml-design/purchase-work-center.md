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
| `/admin/purchase/work-center` | GET | 主页（detail-header：待办总数 + 逾期/临期 pill + 单容器 `#pc-card` + 3 drawer overlay） |
| `/admin/purchase/work-center/demand` | GET | ① 需求业务 tab（`pc_tab_bar` + 视图下拉 + 搜索，`hx-select=#pc-card`） |
| `/admin/purchase/work-center/orders` | GET | ② 订单业务 tab |
| `/admin/purchase/work-center/settlement` | GET | ③ 对账付款业务 tab |
| `/admin/purchase/work-center/returns` | GET | ④ 退货业务 tab |
| `/admin/purchase/work-center/quotations` | GET | ⑤ 供应商报价 tab |
| `/admin/purchase/work-center/misc` | GET | ⑥ 零星请购 tab |
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
| `/batch-convert/{supplier_id}/drawer?demand_ids=` | GET | 批量转单 drawer body（采购明细：同供应商多物料多选 → 一张 PO，调 `get_demands_by_ids` 汇总） |
| `/batch-convert` | POST | 批量转采购单 → `demandChanged, poChanged`（复用 `create_order_from_demands`） |
| `/excel/export/purchase-order?order_id=` | POST | 详情 drawer 单 PO 导出：订单头（单号/供应商/日期/交期/金额/状态/采购员/付款条款/交货地址/备注）+ 全部明细行（行号/物料/数量/单价/金额/已收）。`PurchaseOrderExporter`（`shared/excel/purchase_order_export.rs`）。order_id 经 URL query（同 `/excel/export/bom?bom_id=` 既有模式）。打印复用独立端点 `POPrintPath`（`/admin/purchase/orders/{id}/print`，非本 work-center 路由） |

**HTMX 契约**（tab 模式，对齐 `mes_work_center` — section 外壳 + 单容器 + 顶部业务 tab 栏）：
- card 外壳分离（对齐 MES `render_card_shell`）：首页 `section`（边框/阴影/圆角）持久不替换，内含**标题栏**（图标 + 「采购作业」+ `summary.total()` 件待办 meta）+ 内容 div `#pc-card`；各 card 端点只返回 `#pc-card`（替换内容，外壳 + 标题栏保留）。
- 单容器 `#pc-card`：首页占位 div `hx-trigger="load" hx-target="this" hx-swap="outerHTML"` 懒加载默认 tab（物料汇总）；各端点返回的 `<div id="pc-card">` 自带 `hx-trigger="poChanged/reconChanged/returnChanged/demandChanged from:body"` + `hx-get=自身端点` + `hx-vals`（当前视图/下拉值）+ `hx-include="#pc-filter-form"`（keyword），写操作广播事件后当前 tab 自刷新。
- 顶部业务 tab 栏（`pc_tab_bar`，**7 tab**）：**采购明细 / 物料汇总**（需求拆两 tab，靠 `view` 区分）+ 供应商报价 / 零星请购 / 采购订单 / 对账付款 / 采购退货。选中态 `toggle_cls`（`bg-accent-bg`）+ `tab_badge`（**各业务「全部」计数**，与 card 默认全部查询的数据一致：明细 `demand_detail_total`、汇总 `pending_demand`、订单 `total_orders`、对账 `total_recon`、退货 `total_returns`、报价 `total_quotations`、请购 `total_misc`；Phase 1.9 加这 6 字段到 `PurchaseWorkCenterSummary`，`summary()` 各调一次 `list(default)` count，best-effort 容错）。
- **card 查询逻辑对齐各列表页**（`purchase_<domain>_list.rs`，Phase 1.8）：Params 用 `status: Option<i16>` 默认 None=全部（替代原 `tab` 强制状态分桶），handler `status.and_then(XxxStatus::from_i16)` 传 `list`，状态下拉选项 = 列表页 `status_tabs` TabItem（首项「全部(value="")」）。对账付款保留 `tab=recon/payment` 实体切换下拉 + 状态下拉（选项随实体变：recon 草稿/已确认/已结算，payment 草稿/已核准/已付款/已取消）。报价/请购同理（报价 Draft/Active/Expired/Cancelled；请购 Draft/Approved/Purchasing/Received/Closed/Cancelled）。
- **就地分页**（少即是多，不跳列表页）：各 tab 表格底部 `pagination(base_path, "#pc-card", "#pc-filter-form", total, page, total_pages)`，页码链接 `hx-vals={"page":N}` + `hx-include="#pc-filter-form"` 携带筛选；各端点 `page` 参数驱动 `PageParams::new(page, 10)`。原「查看全部 →」跳转链接 + `info_box` 说明条已移除。
- 行展开 chevron：`hx-get=...row-detail hx-target="this" hx-swap="afterend" _="on click toggle .open on closest <tr/>"`。
- 需求物料行：`hx-get=...demand-rows?product_id= hx-trigger="click once" _="on click toggle .expanded on #pc-demand-toggle-{pid}"`。
- 写操作：事务包裹（`state.pool.begin()...tx.commit()`），返回 `([("HX-Trigger", "poChanged")], Html::empty())`。

**事件命名**：
- `poChanged` — 订单审批/驳回（影响订单业务 tab + tab 栏 badge）
- `reconChanged` — 对账确认/付款审批（影响对账付款业务 tab）
- `returnChanged` — 预留（退货发货 Phase 2 就地化时启用）
- `demandChanged` — 转采购单（影响需求业务 tab）

**drawer**：复用 `render_drawer_overlay`（overlay + `open:` 变体 + Hyperscript 关闭），body 由 `hx-get` 填充。

**详情 drawer 单 PO 打印 + 导出**（Phase 1.11，Issue #200）：
- PO 详情 drawer（`render_po_detail_drawer_body`）头部加 `print_dropdown`（compact）+ 导出按钮。打印复用独立详情页端点 `POPrintPath`（`/admin/purchase/orders/{id}/print`，minijinja `purchase_order` 模板 + `window.print()`），drawer 内隐藏 iframe `#pc-po-print-frame` 触发（`print_dropdown` set src）；模板下拉列 `list_by_document_type("purchase_order")` + 「管理打印模板」入口。
- 导出按钮 `hx-post="/excel/export/purchase-order?order_id={id}"`：order_id 经 URL query 传（同 `/excel/export/bom?bom_id=` 既有模式；`post_export_start` 用 `Query<ExportForm>` 从 URL 解析，非 body）。
- 导出器 `PurchaseOrderExporter`（`abt-core/src/shared/excel/purchase_order_export.rs`）：`new(pool, order_id)`，导出**单个 PO** 的订单头（单号/供应商/订单日期/预计交期/状态/采购员/总金额/付款条款/交货地址/备注，label:value 两列）+ 全部明细行（行号/物料编码/物料名称/数量/单价/金额/已收数量）。注册到 `/excel/export/purchase-order`（`routes/excel.rs`：`ExportForm.order_id`（`empty_as_none`）+ `post_export_start` 权限 `PURCHASE_ORDER read` + `execute_export` 分支）。
- `render_po_print_fragment`（`purchase_order_detail.rs`）是 `print_purchase_order` 渲染主体抽出的共享 helper（单 PO minijinja 渲染，不含 `window.print()`），`POPrintPath` 端点调它。

## 7. Phase 划分

- **Phase 1**：4 只读 card + 锚点条 + 订单审批/付款审批 drawer + 审批/驳回/对账确认/付款审批 4 写操作。
- **Phase 1.5（已实现，原型 `02-work-center.html` 对齐）**：
  - **图标层**：头部 meta chip（日期/本周待办）+ 锚点 chip（4 业务图标）+ 告警 pill（逾期/临期）+ 4 grp-icon + chevron + 物料 mat-ic（紧急度配色）+ ci-row 就绪态 + info-box + hub-link 箭头。
  - **结构层**：grp 分组折叠（`grp-head` + `grp-body`）+ 4 card 行内展开 detail-grid（chevron `hx-swap=afterend`）+ 需求物料卡片化（懒加载 demand-rows）+ 订单收货进度条 + 付款 drawer 三单匹配 ci-row。
  - **abt-core 4 聚合接口**：`get_po_hub_summary` / `check_three_way_match` / `get_settlement_hub_summary` / `get_return_hub_summary`。
- **Phase 1.6（已实现）**：布局从「4 card 上下堆叠 + grp 折叠 + 锚点条」改为「顶部业务 tab + 单容器 `#pc-card`」tab 模式（对齐 `mes_work_center`）——`pc_tab_bar` 进各端点 HTML 随刷新重渲染（badge 实时），事件自刷新保持当前 tab；锚点条退役，逾期/临期 pill 上移 detail-header。各 card 端点保留（未合并单端点 + view），统一容器 id `#pc-card`；二层子 tab 改为状态下拉 select（对齐 MES `wo_status`，单层业务 tab + 下拉筛选）。
- **Phase 1.7（已实现）**：card 样式对齐 MES —— `section` 外壳与 `#pc-card` 内容分离（边框/阴影/圆角持久，不随刷新丢）+ 标题栏（图标 + 「采购作业」+ 待办 meta）；需求拆「采购明细 / 物料汇总」两个独立 tab（5 tab，对齐 MES `detail/material`）；`toggle_cls` 改用 MES `bg-accent-bg` 块状样式；各 tab **就地分页**（移除「查看全部 →」跳转链接和 `info_box`，少即是多 —— 用户在作业中心一屏完成全部工作）。
- **Phase 1.8（已实现）**：card 查询逻辑对齐各列表页 —— Params 改 `status: Option<i16>` 默认全部（替代原 `tab` 强制状态分桶；如退货原默认 `Confirmed` 只显 26 条 → 现默认全部 36 条 = `/admin/purchase/returns`，订单原默认 `PendingApproval` 0 条 → 现全部 453 条 = `/admin/purchase/orders`），状态下拉选项 = 列表页 `status_tabs` TabItem；新增「供应商报价」+「零星请购」2 tab（共 7 tab），报价走 `PurchaseQuotationService::list`，请购走 `state.misc_request_service().list`（工厂 `new_misc_request_service`，`misc_request/mod.rs:10`）。
- **Phase 1.9（已实现）**：tab badge 改用各业务「全部」计数 —— `PurchaseWorkCenterSummary` 加 `total_orders / total_returns / total_quotations / total_misc / total_recon / demand_detail_total` 6 字段，`summary()` 各调一次 `list(default)` count（best-effort），`pc_tab_bar` badge 改用这些字段（与 card 默认全部查询的数据一致：退货 badge 36 = 数据 36，订单 453 = 453）。原 badge 用 `pending_*` 待办计数（与「默认全部」数据不一致）已弃用于 badge；`pending_*` 仍留 header 逾期/临期 pill。
- **Phase 1.10（已实现）**：采购明细 tab（`view=detail`）按供应商归集转单 —— 顶部加供应商搜索控件（`components/supplier_search`：搜全部供应商，store_id=true 选中存 id），选中后需求表切 checkbox 多选模式（`.pc-demand-cb` + 全选 + `.pc-batch-bar` 批量栏，对齐 MES detail_batch_bar），多选同供应商多物料需求 → drawer 确认（供应商只读 + 需求/物料/总量汇总 + 交期/备注）→ `post_batch_convert` 复用 `create_order_from_demands` 生成同一供应商一张 PO（按物料聚合多行）。配套：`DemandPoolQuery.supplier_id`（repo 子查询 `product_id ∈ 该供应商有效报价`，对齐 `compare_by_product` 口径）；`PurchaseDemandService::get_demands_by_ids`（drawer 汇总）；`supplier_search` 加 `display_value` 参数（store_id=true 时 SSR 回显名）。批量栏 JS 与 MES `.demand-cb` 隔离（采购允许同供应商多物料，MES 强制单物料）。性能：summary 15 查询 `tokio::join!` 并发 + `AppState` 内存缓存（TTL 30s，写操作 invalidate）。实现了 Phase 2「转单就地 drawer」的采购明细部分。
- **Phase 1.11（已实现）**：详情 drawer 单 PO 打印 + 导出（Issue #200）—— drawer 头部加 `print_dropdown`（复用 `POPrintPath` 单 PO 打印端点）+ 导出按钮（`PurchaseOrderExporter` 单 PO 头 + 明细）。复用 `print_template` 服务 + `shared/excel` 框架，不改 core Service trait / 数据模型。详见 §6「详情 drawer 单 PO 打印 + 导出」。
- **Phase 2（后续）**：登记收货 / 退货发货 / 创建对账单的就地 drawer；退货物流单号字段（`PurchaseReturn` 现无 `tracking_no`，需 migration）；settlement card「待对账入库」tab（当前仅 draft/payment 两 tab）。
