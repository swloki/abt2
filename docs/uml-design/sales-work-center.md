# 销售作业中心（WorkCenter）设计

> 关联：销售子域（报价 / 销售订单 / 退货 / 月对账单）已齐备，缺一个「销售员/销售内勤一进系统就知道先做什么」的聚合作业页。
> 参照：[`purchase-work-center.md`](./purchase-work-center.md)（业务最对称）、`mes-work-center.md`、`wms-work-center.md`。
> 现状：销售管理只有 4 个独立列表页（报价单 / 销售订单 / 销售退货 / 月对账单）+ 一个销售总览 dashboard，**无作业中心**——待办分散、需逐个进详情页操作。

## 1. 定位

销售是**接单 + 履约 + 回款闭环**的业务域（报价 → 订单 → 发货 → 退货逆向 → 对账 → 收款）。销售岗需要一个**作业中心**，把分散在各列表页的「待处理」状态聚合到一屏，就地确认 / 申请发货 / 审批退货 / 推进对账，不跳详情页。

范式与采购 / MES / WMS work_center 一致：**组件化单端点**（每个 card 一个 GET 端点 + `hx-select` 局部刷新）+ **HX-Trigger 事件联动**（写操作广播，相关 card 自刷新）+ **drawer 就地操作**。

### 与采购作业中心的三处不对称

| # | 采购 | 销售 | 设计处理 |
|---|---|---|---|
| 1 | 有「需求池 / 零星请购」card（提需求方） | 接单方，内部供给需求归 MES/采购作业中心 | 不设需求/请购 card |
| 2 | 对账 + 付款都在采购域（`purchase::reconciliation` + `purchase::payment`） | 对账在销售域（`sales::reconciliation`），**收款核销在财务域**（`fms`） | 对账收款 card 放月对账单操作 + AR 待收款**只读展示**；收款操作仍在财务作业中心 |
| 3 | PO 有审批流（Draft→Submit→Confirm） | 销售订单**无审批流**（Create→Confirm 直接确认） | 订单 card 核心动作是「确认 / 申请发货」，无审批 drawer |
| 4 | AP 立账 source=PurchaseOrder（来料验收立账） | **AR 立账 source=ShippingRequest**（发货立账，非订单） | 订单 AR 聚合用**客户维度**余额（精确反查需多跳，第一阶段近似） |

## 2. SalesWorkCenterService 接口

```rust
#[async_trait]
pub trait SalesWorkCenterService: Send + Sync {
    /// 聚合各业务分组待办计数（首页锚点条 + 各 card tab badge 用）。
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<SalesWorkCenterSummary>;

    /// 销售订单行展开聚合（订单 card row-detail）：发货进度 + 来源链 + AR 台账（客户维度）。
    async fn get_order_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, order_id: i64) -> Result<SalesOrderHubSummary>;

    /// 报价单行展开聚合（报价 card row-detail）：明细 + 可转单状态。
    async fn get_quotation_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, quotation_id: i64) -> Result<QuotationHubSummary>;

    /// 销售退货行展开聚合（退货 card row-detail）：来源 SO + 收货进度。
    async fn get_return_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, return_id: i64) -> Result<SalesReturnHubSummary>;

    /// 对账收发行展开聚合（草稿/待发送对账单、待结算对账单 + 客户 AR 未清）。
    async fn get_settlement_hub_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>, recon_type: SettlementReconType, ref_id: i64) -> Result<SettlementHubSummary>;
}
```

**设计原则**（同采购 / WMS / MES）：WorkCenterService 是**只读聚合层**——既做聚合计数（`summary`），也做行展开详情聚合（`*_hub_summary`）。各 card 列表复用现有 service 的 `list`（按状态过滤）；聚合方法**经 trait 跨域调** sales 各子域 + `ArApService`（fms）+ `CustomerService`（master_data）+ `PickingService`（wms，发货进度），**不直访任何 repo**。细项查询失败 best-effort 容错（返回默认 + `tracing::warn!`），不连累整行/整页。

## 3. SalesWorkCenterSummary model

```rust
pub struct SalesWorkCenterSummary {
    // ── 报价单（QuotationStatus: Draft=1 Sent=2 Accepted=3）──
    pub quotation_draft: u64,          // Draft 待提交
    pub quotation_sent: u64,           // Sent 待客户回复
    pub quotation_accepted: u64,       // Accepted 待转销售订单
    // ── 销售订单（Draft=1 Confirmed=2 ReadyToShip=3 ShippingRequested=8 PartiallyShipped=4）──
    pub order_draft: u64,              // Draft 待确认
    pub order_pending_ship: u64,       // Confirmed + ReadyToShip 待发货
    pub order_shipping: u64,           // ShippingRequested 已申请待仓库拣货
    pub order_partial: u64,            // PartiallyShipped 部分发货
    // ── 销售退货（ReturnStatus: Draft=1 Confirmed=2 Received=3 Inspecting=4）──
    pub return_pending: u64,           // Draft 待确认
    pub return_pending_receive: u64,   // Confirmed 待收货
    pub return_pending_inspect: u64,   // Received 待检验
    // ── 月对账单（ReconciliationStatus: Draft=1 Sent=2 Confirmed=3）──
    pub recon_draft: u64,              // Draft 草稿待发送
    pub recon_sent: u64,               // Sent 待客户确认
    pub recon_confirmed: u64,          // Confirmed 待结算
    // ── AR 联动（fms ArApService，CounterpartyType::Customer）──
    pub ar_overdue_count: u64,         // 逾期应收笔数（due_date < today 且 outstanding > 0）
    pub ar_outstanding_amount: Decimal,// 未清应收余额（Σ amount_outstanding）
    // ── 各业务「全部」计数（tab badge 用，对齐 card 默认全部查询）──
    pub total_quotations: u64,
    pub total_orders: u64,
    pub total_returns: u64,
    pub total_recon: u64,
}
```

`total()` = 报价 3 项 + 订单 4 项 + 退货 3 项 + 对账 3 项之和（**不含** `ar_overdue_count`，避免与对账/订单计数语义重复；AR 逾期单独走 header 告警 pill）。

## 3.5 行展开聚合模型（row-detail）

每个 card 行内展开（chevron `hx-get row-detail` + `hx-swap="afterend"`）调对应 hub_summary，返回 detail-grid。模型定义见 `abt-core/src/sales/work_center/model.rs`：

| 模型 | 方法 | 关键字段 |
|---|---|---|
| `SalesOrderHubSummary` | `get_order_hub_summary(order_id)` | `order` + `customer_name` + `progress{ordered/shipped/open/returned_qty, shipped_pct, item_count}` + `source_chain{quotation_docs[]}` + `ar_summary{ar_amount, received_amount, outstanding}`（客户维度） |
| `QuotationHubSummary` | `get_quotation_hub_summary(quotation_id)` | `quotation` + `customer_name` + `item_count` + `total_amount` + `can_convert_to_so`（Accepted 状态可转） |
| `SalesReturnHubSummary` | `get_return_hub_summary(return_id)` | `return_order` + `customer_name` + `source_so_doc` + `item_count/total_qty` + `status_hint` |
| `SettlementHubSummary` | `get_settlement_hub_summary(recon_type, ref_id)` | `recon_type` + `customer_name` + `recon{doc_number/period/total/confirmed/difference, item_count}` + `ar_outstanding`（客户未清应收） |

`SettlementReconType::parse("draft" | "settle")` 从路径参数解析（对账收款 card 的 2 类行展开：草稿/待发送对账单 vs 待结算对账单 Confirmed）。

> **SalesOrderArSummary 口径**：AR 在发货时由 `ShipmentShippedHandler` 立账（`source_type=ShippingRequest`，非销售订单），无法直接按 order_id 反查。第一阶段用**客户维度** `ArApService::get_party_balance(Customer, order.customer_id)` 取该客户全量未清应收（`outstanding`）+ 已立应收（`ar_amount`）+ 已收核销（`received_amount`）。精确到订单的反查（订单→发货单→AR 多跳）留作后续增强。

## 3.6 跨域依赖（聚合方法消费的共享 / 他域 Service）

| 依赖 | 用途 | 调用方法 |
|---|---|---|
| `ArApService`（fms） | 客户 AR 余额 + AR 待收 list + 逾期计数 | `get_party_balance(Customer, id)`；`list_ledger(party_type=Customer, outstanding_only=true)`；`ledger_summary`（逾期/未清） |
| `CustomerService`（master_data） | 客户名 | `get(customer_id).name`（批量用 `get_by_ids`） |
| `PickingService`（wms） | 销售订单发货进度（可选，精确反查发货单） | `list_items(shipping_request_id)`（第一阶段发货进度直接用 order_items.shipped_qty 聚合，免跨域） |
| `QuotationService` / `SalesOrderService` / `SalesReturnService` / `ReconciliationService` | 各子域 get / list / list_items | 复用既有 trait |

> **跨 crate 边界合规**：`sales::work_center::implt` 调用 `fms::ar_ap` / `master_data::customer` / `wms::picking` 等的 **Service trait**（经工厂获取），符合「abt-core 内部跨模块走 Service trait」约束。反查均经 trait，**不直访 repo**。
>
> **AR 反查契约**：`ArApLedgerFilter` 无 `source_id` 字段（见 `fms-ar-ap.md`），客户 AR 经 `party_type=Customer + party_id` 过滤；逾期/未清经 `ledger_summary` 或 `outstanding_only=true` + post-filter `due_date < today`。

## 4. 各 card「待办」状态边界

| Card | tab / 数据源 | pending 状态（枚举值） | Service |
|---|---|---|---|
| ① 报价单 | 待提交 | `QuotationStatus::Draft` | `QuotationService::list` |
| ① 报价单 | 待客户回复 | `QuotationStatus::Sent` | 同上 |
| ① 报价单 | 待转单 | `QuotationStatus::Accepted` | 同上 |
| ② 销售订单 | 待确认 | `SalesOrderStatus::Draft` | `SalesOrderService::list` |
| ② 销售订单 | 待发货 | `Confirmed` + `ReadyToShip` | 同上 |
| ② 销售订单 | 已申请待拣货 | `ShippingRequested` | 同上 |
| ② 销售订单 | 部分发货 | `PartiallyShipped` | 同上 |
| ③ 销售退货 | 待确认 | `ReturnStatus::Draft` | `SalesReturnService::list` |
| ③ 销售退货 | 待收货 | `ReturnStatus::Confirmed` | 同上 |
| ③ 销售退货 | 待检验 | `ReturnStatus::Received` | 同上 |
| ④ 对账收款 | 草稿待发送 | `ReconciliationStatus::Draft` | `ReconciliationService::list` |
| ④ 对账收款 | 待客户确认 | `ReconciliationStatus::Sent` | 同上 |
| ④ 对账收款 | 待结算 | `ReconciliationStatus::Confirmed` | 同上 |
| ④ 对账收款 | AR 待收（联动） | — | `ArApService::list_ledger(Customer, outstanding_only=true)` |

各 card 列表默认 `status=None`（全部），与各列表页对齐；状态下拉选项 = 各列表页 `status_tabs`。tab badge = 各业务「全部」计数（`total_*`）。

## 5. 实现策略

`SalesWorkCenterServiceImpl::summary`（`abt-core/src/sales/work_center/implt.rs`）：

- 按需工厂获取各 service（`new_sales_order_service(self.pool.clone())` 等，struct 只持 `PgPool`）
- 每项计数调对应 `list(status=..., PageParams::new(1,1))` 取 `total`，经 `cnt()` helper **容错**：单项失败 `tracing::warn!` 后记 0，不连累整页（同采购/MES）
- **AR 逾期 / 未清**：`ArApService::ledger_summary(Customer)` 或 `list_ledger(outstanding_only=true)` 取逾期笔数 + 未清余额，best-effort 容错
- **性能**：各计数查询 `tokio::join!` 并行（同采购 ~15 查询并行 ~30ms）+ `AppState` 内存缓存（TTL 30s，写操作 invalidate）

## 6. 前端（`abt-web`）

**路由**（`/admin/sales/work-center`）：

| 路径 | 方法 | 说明 |
|---|---|---|
| `/admin/sales/work-center` | GET | 主页（detail-header：待办总数 + AR 逾期/未清 pill + 单容器 `#sc-card` + drawer overlay 壳集合） |
| `/admin/sales/work-center/quotations` | GET | ① 报价单 card（`sc_tab_bar` + 状态下拉 + 搜索，`hx-select=#sc-card`） |
| `/admin/sales/work-center/orders` | GET | ② 销售订单 card |
| `/admin/sales/work-center/returns` | GET | ③ 销售退货 card |
| `/admin/sales/work-center/settlement` | GET | ④ 对账收款 card（月对账单 + AR 待收子 tab） |
| `/quotations/{id}/row-detail` | GET | 报价行展开 detail-grid（调 `get_quotation_hub_summary`） |
| `/orders/{id}/row-detail` | GET | 订单行展开（调 `get_order_hub_summary`：发货进度 + AR 台账） |
| `/returns/{id}/row-detail` | GET | 退货行展开（调 `get_return_hub_summary`） |
| `/settlement/{recon_type}/{ref_id}/row-detail` | GET | 对账行展开（draft / settle 两套 grid） |
| `/quotations/{id}/detail-drawer` | GET | 报价详情 drawer body（查看 + 状态操作） |
| `/orders/{id}/detail-drawer` | GET | 订单详情 drawer body |
| `/returns/{id}/detail-drawer` | GET | 退货详情 drawer body |
| `/settlement/{id}/detail-drawer` | GET | 对账详情 drawer body |
| `/quotations/create-drawer` | GET+POST | 就地新建报价（可选） |
| `/returns/create-drawer` | GET+POST | 就地新建退货 |
| `/reconciliations/create-drawer` | GET+POST | 就地新建对账单 |
| `/quotations/{id}/{submit\|accept\|reject\|expire\|to-so}` | POST | 报价写操作 → `salesQuotationChanged`（to-so 额外 `soChanged`） |
| `/orders/{id}/{confirm\|request-ship\|cancel}` | POST | 订单写操作 → `soChanged` |
| `/returns/{id}/{approve\|receive\|inspect\|complete\|reject\|cancel}` | POST | 退货写操作 → `salesReturnChanged` |
| `/reconciliations/{id}/{send\|confirm\|dispute\|settle}` | POST | 对账写操作 → `salesReconChanged` |

**申请发货**：复用既有 `sales_order_detail::request_shipment` 的 service 逻辑（`RequestShipPath` 已存在，migration 074）。作业中心订单 card / detail drawer 加「申请发货」按钮，POST 到 `/orders/{id}/request-ship`（作业中心端点包事务 + 广播 `soChanged`）。

**HTMX 契约**（tab 模式，对齐 `purchase_work_center`）：
- card 外壳分离：首页 `section`（边框/阴影/圆角）持久不替换，内含**标题栏**（图标 + 「销售作业」+ `summary.total()` 件待办 meta + 「新建」按钮）+ 内容 div `#sc-card`；各 card 端点只返回 `#sc-card`（替换内容，外壳 + 标题栏保留）。
- 单容器 `#sc-card`：首页占位 div `hx-trigger="load" hx-target="this" hx-swap="outerHTML"` 懒加载默认 card（报价单或订单）；各端点返回的 `<div id="sc-card">` 自带 `hx-trigger="soChanged/salesQuotationChanged/salesReturnChanged/salesReconChanged from:body"` + `hx-get=自身端点` + `hx-vals`（当前状态/视图）+ `hx-include="#sc-filter-form"`，写操作广播事件后当前 card 自刷新。
- 顶部业务 tab 栏（`sc_tab_bar`，**4 tab**）：报价单 / 销售订单 / 销售退货 / 对账收款。选中态 `toggle_cls`（`bg-accent-bg`）+ `tab_badge`（各业务「全部」计数 `total_*`）。
- card 查询逻辑对齐各列表页：Params 用 `status: Option<i16>` 默认 None=全部，handler `status.and_then(XxxStatus::from_i16)` 传 `list`，状态下拉选项 = 列表页 `status_tabs`。
- 就地分页：各 card 表格底部 `pagination(base_path, "#sc-card", "#sc-filter-form", total, page, total_pages)`。
- 行展开 chevron：`hx-get=...row-detail hx-target="this" hx-swap="afterend" _="on click toggle .open on closest <tr/>"`。
- 写操作：事务包裹（`state.pool.begin()...tx.commit()`），返回 `([("HX-Trigger", "soChanged")], Html::empty())`。
- **主从表 + 斑马纹**：订单/对账明细用 rowspan 主从表范式（对齐 a4804233 提交的 purchase/wms work_center 明细）。

**事件命名**（全局唯一，加 `s`/`sales` 前缀避免与采购 `poChanged`/`reconChanged`/`returnChanged` 撞名）：
- `soChanged` — 销售订单写操作（confirm / request-ship / cancel）
- `salesQuotationChanged` — 报价单写操作（submit / accept / reject / expire / to-so）
- `salesReturnChanged` — 销售退货写操作
- `salesReconChanged` — 月对账单写操作

**drawer**：复用 `render_drawer_overlay`（overlay + `open:` 变体 + Hyperscript 关闭），body 由 `hx-get` 填充。打开/关闭事件契约对齐采购：触发按钮 `hx-get` 填充 body + `on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #overlay`；提交 form `on 'htmx:afterRequest'[detail.xhr.responseText.length == 0 and detail.elt is me] remove .open`（[[feedback-create-drawer-close-hx-redirect]] / [[feedback-drawer-close-pitfalls]]）。

## 7. Phase 划分（实现状态）

- **Phase 1（只读聚合 + 骨架）✅ 已实现**：`SalesWorkCenterService`（summary + 4 hub_summary）+ 首页 + 4 card 列表 + tab 栏 + 行展开聚合 + summary 缓存 + 导航入口。
- **Phase 2（就地写操作 drawer）✅ 已实现（核心流转）**：4 域 detail drawer（单号点击打开，查看 + 状态操作）+ 11 写操作 handler（报价 submit/accept/to-so、订单 confirm/cancel、退货 approve/receive/complete/cancel、对账 send/confirm/settle），事务包裹 + invalidate + HX-Trigger 广播。
- **Phase 2.1（补全流转 + 申请发货）✅ 已实现**：补全报价 reject/expire、退货 inspect/reject、对账 dispute；订单「申请发货」复用 `RequestShipPath` modal（建发货单 + 订单 ShippingRequested），订单 drawer 按钮 `hx-get` 触发。
- **后续增强（未实现）**：
  - ✅ 对账创建 drawer（客户 + 期间，无行项目）
  - ✅ 报价/退货创建 drawer（`hx-select` 复用 create 页表单片段嵌入 drawer，提交跳详情页；零代码重写）
  - ✅ 对账 reopen / force_settle（重开 + 强制结算，Disputed 分支）
  - 销售订单 AR 精确反查（需 `ArApService` 加按 source_ids 查询 + `PickingService` 按 order_id 查发货单，abt-core 改动大；当前用客户维度 `get_party_balance` 近似，展示客户全部 AR）
  - 对账收款 card 收款操作（收款核销在财务域，与财务作业中心边界确认后再接入）
  - 详情 drawer 单据打印/导出（对齐采购 Issue #200）
