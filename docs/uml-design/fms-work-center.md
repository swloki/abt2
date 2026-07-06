# 财务作业中心（FMS WorkCenter）设计

> 关联：fms 各子域（应收应付台账 / 出纳日记账 / 调整 / 核销）已齐备，缺一个「财务岗一进系统就知道先做什么」的聚合作业页。
> 参照：`purchase-work-center.md`、`mes-work_center`、`wms-work-center.md`（组件化单端点模式）。
> 现状：原 `fms_dashboard`（`/admin/fms`）已于 2026-07-06 移除（现金流仪表盘与作业中心定位重叠，财务岗入口统一到工作中心）；`/admin/fms` 根路径重定向到 `/admin/fms/work-center`。

## 1. 定位

财务是**业财一体后的往来闭环**域（销售发货/采购入库/委外收货自动立 AR/AP 台账 → 收付款登记 → 核销 → 结清）。财务岗需要一个**作业中心**，把分散在各列表页的「待处理」状态聚合到一屏，就地登记收付款 / 确认 / 核销，不跳详情页。

范式与采购 / MES / WMS work_center 一致：**组件化单端点**（每个 tab 一个 GET 端点 + `hx-select="#fc-card"` 局部刷新）+ **HX-Trigger 事件联动**（写操作广播，相关 tab 自刷新）+ **drawer 就地操作**。

工作中心定位为财务域**主作业台**：既聚合待办，也承载全量台账查询（对齐采购工作中心 `get_orders_card` 全量 + 筛选 + 分页范式）。AR/AP 台账、AR/AP 调整、核销记录独立列表页的独有能力（summary 卡片 / 高级筛选 / 导出 / 撤销核销 / 新建调整入口 / 产品明细 drawer）已于 2026-07-06 合并进各 tab，独立页（`fms_dashboard` / `fms_ar_ledger` / `fms_ap_ledger` / `fms_adjustment_list` / `fms_settlement`）及对应路由、菜单已删除；调整创建页 `fms_adjustment_create` 保留（入口迁到工作中心调整 tab）。

## 2. FmsWorkCenterService 接口

```rust
// abt-core/src/fms/work_center/service.rs
use async_trait::async_trait;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};
use super::model::FmsWorkCenterSummary;

/// 财务作业中心聚合服务（只读视图，写操作复用 fms 各域既有 Service）。
#[async_trait]
pub trait FmsWorkCenterService: Send + Sync {
    /// 聚合各 tab 待办计数 + 顶栏 pill（逾期/临期金额）。首页锚点 + tab badge + pill 用。
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<FmsWorkCenterSummary>;
}
```

**设计原则**（同采购版）：WorkCenterService 是**只读聚合层**。struct 只持 `PgPool`，方法体通过按需工厂获取 fms 各 service（`new_ar_ap_service(self.pool.clone())` 等），**不直访任何 repo**。细项查询失败 best-effort 容错（返回默认 + `tracing::warn!`），不连累整页（同 `summary` 哲学）。

**Phase 1 零新增 trait 方法、零 migration**：`summary()` 内部全部调现有接口。行展开复用 `ArApService::get_ledger_detail(id)`（已有），无需新 hub_summary。

**跨域依赖**（聚合方法消费的 fms Service，全部本域内调用）：

| 依赖 | 用途 | 调用方法 |
|---|---|---|
| `ArApService`（fms/ar_ap） | AR/AP 未清金额 + 笔数 + 核销记录数 | `ledger_summary(filter)`；`list_ledger(filter, page).total`；`list_settlements(filter, page).total` |
| `CashJournalService`（fms/cash_journal） | 待确认 Draft 笔数 | `list(filter{status:[Draft]}, page).total` |

## 3. FmsWorkCenterSummary model

```rust
// abt-core/src/fms/work_center/model.rs
use rust_decimal::Decimal;

/// 财务作业中心待办汇总（顶栏 pill + 各 tab badge 用）。
///
/// 各项查询失败按 0 / Decimal::ZERO 容错，不连累整页（同采购 / MES work_center）。
/// 写操作复用 fms 各域既有 Service，此处只做只读聚合。
#[derive(Debug, Clone, Default)]
pub struct FmsWorkCenterSummary {
    // ── 应收（AR，party_type=Customer）──
    pub ar_outstanding_amount: Decimal,   // 未清金额 ← ledger_summary(Customer).total_outstanding
    pub ar_overdue_amount: Decimal,       // 逾期金额 ← .total_overdue
    pub ar_due_soon_amount: Decimal,      // 7 天内到期 ← .due_within_7d
    pub ar_outstanding_count: u64,        // 未清笔数 ← list_ledger(outstanding_only).total
    // ── 应付（AP，party_type=Supplier）── 对称
    pub ap_outstanding_amount: Decimal,
    pub ap_overdue_amount: Decimal,
    pub ap_due_soon_amount: Decimal,
    pub ap_outstanding_count: u64,
    // ── 出纳 ──
    pub journal_draft_count: u64,         // 待确认笔数 ← cash_journal.list(status=[Draft]).total
    // ── 核销 ──
    pub settlement_total: u64,            // 核销记录总数 ← list_settlements(default).total
}

impl FmsWorkCenterSummary {
    /// 顶栏「待办总数」pill：AR/AP 未清笔数 + 待确认笔数（不含金额，避免与 pill 重复）。
    pub fn total(&self) -> u64 {
        self.ar_outstanding_count + self.ap_outstanding_count + self.journal_draft_count
    }
    /// 顶栏红 pill：AR+AP 逾期金额合计。
    pub fn total_overdue(&self) -> Decimal { self.ar_overdue_amount + self.ap_overdue_amount }
    /// 顶栏黄 pill：AR+AP 7 天内到期金额合计。
    pub fn total_due_soon(&self) -> Decimal { self.ar_due_soon_amount + self.ap_due_soon_amount }
}
```

## 4. 各 tab「待办」边界

| Card（tab） | 端点 / TypedPath | 数据源（复用现有 service） | 默认筛选 | 就地操作 |
|---|---|---|---|---|
| ① 应收待收 | `FcReceivablesPath` `/receivables` | `ar_ap.list_ledger` + `ledger_summary(Customer)` | `outstanding_only=true`, party_type=Customer | 登记收款 drawer、手动核销 drawer、行展开明细（`get_ledger_detail`） |
| ② 应付待付 | `FcPayablesPath` `/payables` | `ar_ap.list_ledger` + `ledger_summary(Supplier)` | `outstanding_only=true`, party_type=Supplier | 登记付款 drawer、手动核销 drawer、行展开 |
| ③ 出纳待确认 | `FcJournalsPath` `/journals` | `cash_journal.list` | `status=[Draft]` | 确认 drawer（`confirm`）、查看明细跳 `/journals/{id}` |
| ④ 核销 | `FcSettlementsPath` `/settlements` | `ar_ap.list_settlements` | 全部 | 反核销（`unsettle`）、查看核销记录 |

**状态下拉**（对齐采购 Phase 1.8「单层业务 tab + 下拉筛选」）：① ② 加「全部 / 只看未清 / 逾期 / 7 天内到期」下拉；③ 加「全部 / 草稿 / 已确认 / 已取消」下拉。

## 5. 实现策略

`FmsWorkCenterServiceImpl::summary`（`abt-core/src/fms/work_center/implt.rs`）：

- 按需工厂获取各 service（`new_ar_ap_service(self.pool.clone())` / `new_cash_journal_service(self.pool.clone())`，struct 只持 `PgPool`）
- 6 个查询 `tokio::join!` 并发，各自从 pool 获连接不阻塞，总耗时 ≈ max(单次)
  - 2× `ledger_summary(ArApLedgerFilter{ party_type: Some(Customer/Supplier), ..default })` → 8 个金额字段
  - 2× `list_ledger(filter{ party_type, outstanding_only:true }, PageParams::new(1,1)).total` → 2 个未清笔数
  - 1× `cash_journal.list(CashJournalFilter{ status: vec![Draft], ..default }, PageParams::new(1,1)).total` → journal_draft_count
  - 1× `list_settlements(SettlementFilter::default(), PageParams::new(1,1)).total` → settlement_total
- 每项 `cnt()` / `amt()` helper 容错：单项查询失败 `tracing::warn!` 后记 0/`Decimal::ZERO`，不连累整页 —— 照搬采购版 `implt.rs:291-299, 317-378`

## 6. 前端（`abt-web`）

**路由表**（`routes/fms_work_center.rs`，TypedPath 命名 `Fc*`，derive `TypedPath, Deserialize, Clone` **不加 Serialize**，前缀 `/admin/fms/work-center`）：

| 路径 | 方法 | 说明 |
|---|---|---|
| `/admin/fms/work-center` | GET | 主页（detail-header：待办总数 + 逾期/临期 pill + section 外壳 + `#fc-card` 占位 + drawer overlay 预渲染） |
| `/admin/fms/work-center/receivables` | GET | ① 应收 tab（`fc_tab_bar` + 状态下拉 + 搜索，`hx-select=#fc-card`） |
| `/admin/fms/work-center/payables` | GET | ② 应付 tab |
| `/admin/fms/work-center/journals` | GET | ③ 出纳待确认 tab |
| `/admin/fms/work-center/settlements` | GET | ④ 核销 tab |
| `/receivables/{id}/receipt-drawer` | GET | 登记收款 drawer body（`CreateCashJournalReq` SalesReceipt，source=StockShipment） |
| `/payables/{id}/payment-drawer` | GET | 登记付款 drawer body（PurchasePayment，source=StockReceipt） |
| `/journals/{id}/confirm-drawer` | GET | 确认收付款 drawer body |
| `/ledger/{party_type}/{party_id}/settle-drawer` | GET | 手动核销 drawer body（`list_open_invoices` + `list_unapplied_payments` + `SettleReq`） |
| `/ledger/{id}/detail` | GET | 台账行展开（`ar_ap.get_ledger_detail` → 产品明细） |
| `/journals/{id}/confirm` | POST | 确认 → `HX-Trigger: journalChanged` |
| `/settlements/settle` | POST | 核销 → `HX-Trigger: settlementChanged` |
| `/settlements/{id}/unsettle` | POST | 反核销 → `settlementChanged` |
| `/journals/receipt` | POST | 创建收款 → `journalChanged` |
| `/journals/payment` | POST | 创建付款 → `journalChanged` |

**路由注册**：`routes/mod.rs:64` 加 `pub mod fms_work_center;`，`:142` 后加 `.merge(fms_work_center::router())`。

**HTMX 契约**（tab 模式，完全对齐采购 `#pc-card`）：

- **card 外壳分离**（对齐采购 `render_card_shell` / MES）：首页 `section`（边框/阴影/圆角）持久不替换，内含**标题栏**（图标 + 「财务作业」+ `summary.total()` 件待办 meta）+ 内容 div `#fc-card`；各 tab 端点只返回 `#fc-card`（替换内容，外壳 + 标题栏保留）。
- **单容器 `#fc-card`**：首页占位 div `hx-trigger="load" hx-target="this" hx-swap="outerHTML"` 懒加载默认 tab（应收）；各端点返回的 `<div id="fc-card">` 自带 `hx-trigger="journalChanged from:body, settlementChanged from:body, arAdjustmentChanged from:body, apAdjustmentChanged from:body"` + `hx-get=自身端点` + `hx-vals`（当前下拉值）+ `hx-include="#fc-filter-form"`（keyword）。写操作广播事件后当前 tab 自刷新。
- **顶部业务 tab 栏**（`fc_tab_bar`，**4 tab**）：应收待收 / 应付待付 / 出纳待确认 / 核销。选中态 `toggle_cls`（`bg-accent-bg` 块状）+ `tab_badge`（应收 `ar_outstanding_count`、应付 `ap_outstanding_count`、出纳 `journal_draft_count`、核销 `settlement_total`）。
- **card 查询逻辑对齐各列表页**：Params 用 `status: Option<i16>` / `outstanding_only: bool` 默认全部；状态下拉选项对齐 `fms_ar_ledger` / `fms_ap_ledger` 的筛选口径。
- **就地分页**：`pagination(base_path, "#fc-card", "#fc-filter-form", total, page, total_pages)`。
- **写操作**：事务包裹（`state.pool.begin()...tx.commit()`），返回 `([("HX-Trigger", "xxxChanged")], Html::empty())`，commit 后调 `invalidate_fms_summary(&state)`。
- **drawer**：复用 `components::overlay::drawer_shell(id, width_class, inner)`；首页预渲染 overlay（`fc-receipt-overlay` / `fc-payment-overlay` / `fc-confirm-overlay` / `fc-settle-overlay` / `fc-ledger-detail-overlay`，均 `fc-` 前缀避冲突）；body 由 `hx-get` 填充，form `hx-swap="none"` + `_="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #fc-xxx-overlay then call showToast(...)"`。
- **创建 drawer 守卫**（含子请求时）：`detail.xhr.responseText.length == 0 and detail.xhr.status < 400`（照搬采购版 `get_po_create_drawer`）—— 校验失败返回非空 form 不误关，成功返回空 body 才关。

**事件命名**（新增前已 grep 全仓确认无重名）：

- `journalChanged` — 收付款确认 / 创建（`confirm` / create receipt/payment）
- `settlementChanged` — 核销 / 反核销（`settle` / `unsettle`）
- `arAdjustmentChanged` — 应收调整创建（`create_ar`）
- `apAdjustmentChanged` — 应付调整创建（`create_ap`）

`#fc-card` 监听全部 4 个事件自刷新；顶栏 pill 由首页 `#fc-summary-header` 监听同事件 oob 重渲染（`hx-select-oob`）。

**缓存**（照搬采购版，`state.rs`）：

- 新增字段 `pub fms_summary_cache: Arc<RwLock<Option<(Instant, FmsWorkCenterSummary)>>>`（初始化 `Arc::new(RwLock::new(None))`，两处 AppState 构造：`:179` 和 `:613`）
- 新增工厂 `pub fn fms_work_center_service(&self) -> impl abt_core::fms::work_center::FmsWorkCenterService { abt_core::fms::work_center::new_fms_work_center_service(self.pool.clone()) }`
- `cached_summary`（`SUMMARY_TTL_SECS = 30`，读路径：命中返回 / miss 算一次回填）+ `invalidate_fms_summary`（每个写 handler commit 后调）—— 双保险：主动失效 + TTL 兜底。

**permission**：所有 GET 端点 `#[require_permission("FMS", "read")]`；写操作 `#[require_permission("FMS", "update")]`（创建用 `create`）。

**sidebar**（`layout/sidebar.rs:232`）：财务管理 `items[0]` 插入 `NavItem { name: "财务作业中心", path: "/admin/fms/work-center", icon: NavIcon::Grid, permission: Some(("FMS", "read")) }`（在「财务总览」之前，对齐采购/MES/WMS 作业中心均为各自 items 首位 + Grid 图标）。

## 7. 现有 fms 页面改造（HX-Redirect → HX-Trigger）

| handler | 现状 | 改造 |
|---|---|---|
| `fms_journal_detail.rs:179 confirm` | 有事务，`HX-Redirect` 详情页 | 改 `HX-Trigger: journalChanged` + 空 body；详情页最外层 div 加 `id="journal-detail-card"` + `hx-trigger="journalChanged from:body"` 自刷新 |
| `fms_settlement.rs:148 unsettle` | **无事务** + `let _ =` **吞错误 bug**，手动重渲染列表 | 补 `pool.begin()...commit()`；`let _ =` 改 `?`（失败走全局 toast）；返回 `HX-Trigger: settlementChanged` + 空 body；`#settlement-data-card` 加自刷新 |
| `fms_adjustment_create.rs:356 create_ar/create_ap` | **无事务**，`HX-Redirect` 列表 | 补事务；返回 `HX-Trigger: arAdjustmentChanged` / `apAdjustmentChanged`；列表页容器自刷新 |
| `fms_journal_create.rs:133 create` | 有事务，`HX-Redirect` 列表 | 改 `HX-Trigger: journalChanged`；`#journal-data-card` 加自刷新 |
| `fms_journal_list.rs #journal-data-card` | — | 合并触发器：`hx-trigger="change, keyup changed delay:300ms from:.search-input, journalChanged from:body"` |
| `fms_adjustment_list.rs #data-card` | id 与 settlement 同名 | **id 语义化**为 `#ar-adjustment-data-card` / `#ap-adjustment-data-card`；加 `ar(ap)AdjustmentChanged` 自刷新 |
| `fms_settlement.rs #data-card` | id 与 adjustment 同名 | **id 语义化**为 `#settlement-data-card`；加 `settlementChanged` 自刷新 |
| `fms_ar_ledger.rs` / `fms_ap_ledger.rs` | — | （Phase 4 联动）监听 `settlementChanged` / `ar(ap)AdjustmentChanged` / `journalChanged` 自刷新余额 |

**HX-Trigger 响应头模板**（禁 `format!` 拼值，用 `serde_json::json!`）：

```rust
// 单事件
Ok(([("HX-Trigger", r#"{"journalChanged":null}"#)], Html(String::new())))
// 多事件 / 带 payload
Ok(([("HX-Trigger", serde_json::json!({"journalChanged": null, "showToast": "已确认"}).to_string())], Html(String::new())))
```

## 8. Phase 划分

- **Phase 1（只读聚合 + 骨架）**：`abt-core/src/fms/work_center/{mod,service,implt,model}.rs`（summary 聚合，零新增 trait 方法）+ `state.rs`（缓存字段 + 工厂）+ `routes/fms_work_center.rs` + sidebar + 首页外壳 + 4 只读 tab + 顶栏 pill + 30s 缓存 + invalidate。
- **Phase 2（就地操作 drawer）**：登记收款 / 登记付款 / 确认 / 手动核销 4 drawer（GET body + POST 写 handler，事务 + HX-Trigger + invalidate）+ 台账行展开（`get_ledger_detail`）+ `#fc-card` 监听 4 事件自刷新。
- **Phase 3（现有页面事件改造）**：`confirm` / `unsettle`（补事务 + 修吞错误 bug）/ `create_ar` / `create_ap`（补事务）/ `create journal` 全部从 HX-Redirect 改 HX-Trigger；列表页容器加自刷新；`#data-card` id 语义化（消除 adjustment/settlement 同名冲突）。
- **Phase 4（联动 + 收尾）**：`fms_ar_ledger` / `fms_ap_ledger` 监听事件自刷新余额；设计文档与代码双向同步自检。

## 9. 复用范本（照搬，给 file:line）

- 工作中心首页 / tab / drawer / 缓存 / 写 handler 范本：`abt-web/src/pages/purchase_work_center.rs:72-167 / 270-327 / 2607-2681 / 2760-2791 / 3525-3617`
- Service trait + 工厂 + Summary 范本：`abt-core/src/purchase/work_center/{service.rs, mod.rs, model.rs, implt.rs}`
- `drawer_shell`：`abt-web/src/components/overlay.rs:45-55`（`drawer_shell(id, width_class, inner)`）
- `pagination`：`abt-web/src/components/pagination.rs::pagination(base_path, target_sel, form_sel, total, page, total_pages)`
- 状态下拉 / `status_tabs`：`abt-web/src/components/tabs.rs`

## 关联文档

- [`purchase-work-center.md`](purchase-work-center.md) — 采购作业中心（范式直接来源）
- [`fms-ar-ap.md`](fms-ar-ap.md) — 应收应付台账（业财一体立账 / 核销契约）
- [`../.omp/rules/htmx-patterns.md`](../.omp/rules/htmx-patterns.md) §2 列表页单端点 / §4 HX-Trigger 联动
