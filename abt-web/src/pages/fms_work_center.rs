//! 财务作业中心 — 应收 / 应付 / 出纳 / 核销 四 tab 聚合工作台。
//!
//! 架构（组件化单端点模式，同 purchase_work_center）：
//! - 首页内联渲染 section 外壳 + `#fc-card` 占位 div，`hx-trigger="load"` 拉默认 tab（应收）；
//! - 每个 tab 一个 GET 端点，tab 内筛选/分页走该端点 + `hx-select="#fc-card"` 局部刷新；
//! - 写操作（登记收付款/确认/核销，Phase 2）POST 广播 `HX-Trigger`（journalChanged /
//!   settlementChanged / arAdjustmentChanged / apAdjustmentChanged），`#fc-card` 声明
//!   `hx-trigger="... from:body"` 自刷新；顶栏 pill 监听同事件 oob 重渲染。
//! - drawer 就地操作（Phase 2）：登记收款/付款、确认、手动核销。

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::NaiveDate;
use maud::{html, Markup, PreEscaped};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::fms::adjustment::{
    model::{ArApAdjustment, CreateAdjustmentReq}, AdjustmentDirection, AdjustmentFilter, AdjustmentRow,
    AdjustmentService,
};
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::fms::ar_ap::{
    ArApLedgerFilter, ArApLedgerRow, ArApService, ArApSettlement, LedgerDetailItem, LedgerSummary,
    OpenInvoice, SettlementFilter, SettleReq, UnappliedPayment,
};
use abt_core::fms::cash_journal::{CashJournalService, CreateCashJournalReq};
use abt_core::fms::enums::{CashDirection, CounterpartyType, JournalType};
use abt_core::fms::work_center::{FmsWorkCenterService, FmsWorkCenterSummary};
use abt_core::shared::types::context::ServiceContext;
use abt_core::shared::enums::DocumentType;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::{PageParams, PgExecutor};

use std::time::Instant;

use crate::components::{entity_picker, icon};
use crate::components::entity_picker::EntityPickerConfig;
use crate::components::overlay::drawer_shell;
use crate::components::pagination::pagination;
use crate::routes::fms::JournalSearchCpPath;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms_work_center::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// =============================================================================
// 首页
// =============================================================================

#[require_permission("FMS", "read")]
pub async fn get_work_center(_path: FmsWorkCenterPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;

    let content = html! {
        // detail-header：标题 + 待办总数 + 逾期/临期 pill
        (fc_summary_header(&summary))
        // card 外壳（section）与内容（#fc-card）分离，对齐 purchase_work_center：
        // 标题栏（图标 + 标题 + meta）持久；#fc-card 由各 tab 端点返回替换。
        section class="bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-3 px-5 py-3 border-b border-border-soft" {
                div class="w-7 h-7 rounded-md grid place-items-center bg-accent-bg text-accent shrink-0" {
                    (icon::currency_icon("w-[18px] h-[18px]"))
                }
                span class="font-semibold text-fg shrink-0" { "财务作业" }
                span class="text-xs text-muted font-mono flex-1 truncate" {
                    (summary.total()) " 件待办 · 应收 / 应付 / 出纳 / 核销 一屏处理"
                }
            }
            // #fc-card 占位：load 时拉默认 tab（应收）。各 tab 端点返回的 div 也用 id="fc-card"，
            // 自带 hx-trigger 监听写操作广播事件，实现 tab 自刷新。
            div id="fc-card"
                hx-get=(FcReceivablesPath::PATH)
                hx-trigger="load"
                hx-target="this" hx-swap="outerHTML" {
                "加载中…"
            }
        }
        // drawer overlay 预渲染（就地操作；body 由各 drawer 端点 hx-get 填充）
        (render_drawer_overlay("fc-receipt-overlay", "fc-receipt-drawer", "fc-receipt-drawer-body", "登记收款", "w-[520px] max-w-[92vw]"))
        (render_drawer_overlay("fc-payment-overlay", "fc-payment-drawer", "fc-payment-drawer-body", "登记付款", "w-[520px] max-w-[92vw]"))
        (render_drawer_overlay("fc-settle-overlay", "fc-settle-drawer", "fc-settle-drawer-body", "手动核销", "w-[680px] max-w-[94vw]"))
        (render_drawer_overlay("fc-ledger-detail-overlay", "fc-ledger-detail-drawer", "fc-ledger-detail-drawer-body", "台账明细", "w-[760px] max-w-[94vw]"))
        (render_drawer_overlay("fc-adjustment-overlay", "fc-adjustment-drawer", "fc-adjustment-drawer-body", "新建调整", "w-[560px] max-w-[92vw]"))
        (render_drawer_overlay("fc-adjustment-detail-overlay", "fc-adjustment-detail-drawer", "fc-adjustment-detail-drawer-body", "调整单详情", "w-[560px] max-w-[92vw]"))
    };

    let page_html = admin_page(
        is_htmx,
        "财务作业中心",
        &claims,
        "finance",
        FmsWorkCenterPath::PATH,
        "财务管理",
        Some("财务作业中心"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// =============================================================================
// Tab 端点
// =============================================================================

// ── ① 应收待收（AR）/ ② 应付待付（AP）── 台账全量查询（summary 卡片 + 高级筛选 + 导出）

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LedgerCardParams {
    /// "all" = 全部；其他/缺省 = 只看未清
    #[serde(default, deserialize_with = "empty_as_none")]
    pub filter: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub product_code: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub product_name: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub doc_no: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub rep_name: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub start_date: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub end_date: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("FMS", "read")]
pub async fn get_receivables_card(
    _path: FcReceivablesPath,
    ctx: RequestContext,
    Query(p): Query<LedgerCardParams>,
) -> Result<Html<String>> {
    ledger_card(
        ctx, p, CounterpartyType::Customer, "receivables", FcReceivablesPath::PATH,
        "ar-ledger-detail", &["XIAOSHOU"], true,
    )
    .await
}

#[require_permission("FMS", "read")]
pub async fn get_payables_card(
    _path: FcPayablesPath,
    ctx: RequestContext,
    Query(p): Query<LedgerCardParams>,
) -> Result<Html<String>> {
    ledger_card(
        ctx, p, CounterpartyType::Supplier, "payables", FcPayablesPath::PATH,
        "ap-ledger-detail", &["CAIGOU", "SHENGCHAN"], false,
    )
    .await
}

/// AR/AP 台账 tab 共用渲染：顶栏 tab + summary 卡片 + 高级筛选 + 表格 + 分页。
async fn ledger_card(
    ctx: RequestContext,
    p: LedgerCardParams,
    party_type: CounterpartyType,
    active: &str,
    path: &'static str,
    export_type: &'static str,
    buyers_dept: &[&str],
    is_receivable: bool,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wc_summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let outstanding_only = p.filter.as_deref() != Some("all");
    let page = p.page.unwrap_or(1);
    let filter = ArApLedgerFilter {
        party_type: Some(party_type),
        outstanding_only,
        keyword: p.keyword.clone(),
        doc_no: p.doc_no.clone(),
        product_code: p.product_code.clone(),
        product_name: p.product_name.clone(),
        rep_name: p.rep_name.clone(),
        start_date: p.start_date.as_deref().and_then(|s| s.trim().parse().ok()),
        end_date: p.end_date.as_deref().and_then(|s| s.trim().parse().ok()),
        ..Default::default()
    };
    let ledger_sum = state
        .ar_ap_service()
        .ledger_summary(&service_ctx, &mut conn, filter.clone())
        .await
        .unwrap_or_default();
    let result = state
        .ar_ap_service()
        .list_ledger(&service_ctx, &mut conn, filter, PageParams::new(page, 10))
        .await?;
    let buyers: Vec<String> = state
        .user_service()
        .list_users_by_departments(&service_ctx, &mut conn, buyers_dept)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|u: &abt_core::shared::identity::model::UserWithRoles| u.user.is_active)
        .filter_map(|u| u.user.display_name)
        .filter(|n: &String| !n.is_empty())
        .collect();
    let today = chrono::Utc::now().date_naive();
    Ok(Html(
        html! {
            div id="fc-card"
                hx-get=(path)
                hx-trigger="journalChanged from:body, settlementChanged from:body, arAdjustmentChanged from:body, apAdjustmentChanged from:body"
                hx-include="#fc-filter-form"
                hx-target="this" hx-select="#fc-card" hx-swap="outerHTML" {
                (fc_tab_bar(active, &wc_summary))
                (ledger_summary_cards(&ledger_sum, is_receivable))
                (ledger_filter_bar(path, &p, outstanding_only, &buyers, export_type))
                (ledger_table(&result.items, today, is_receivable))
                (pagination(path, "#fc-card", "#fc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ③ 出纳待确认 ──

// ── ③ 应收调整 / ④ 应付调整（adjustment，创建即过账，列表查看）──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AdjustmentCardParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("FMS", "read")]
pub async fn get_ar_adjustments_card(
    _path: FcArAdjustmentsPath,
    ctx: RequestContext,
    Query(p): Query<AdjustmentCardParams>,
) -> Result<Html<String>> {
    adjustments_card(ctx, p, CounterpartyType::Customer, "ar-adjustments", FcArAdjustmentsPath::PATH).await
}

#[require_permission("FMS", "read")]
pub async fn get_ap_adjustments_card(
    _path: FcApAdjustmentsPath,
    ctx: RequestContext,
    Query(p): Query<AdjustmentCardParams>,
) -> Result<Html<String>> {
    adjustments_card(ctx, p, CounterpartyType::Supplier, "ap-adjustments", FcApAdjustmentsPath::PATH).await
}

async fn adjustments_card(
    ctx: RequestContext,
    p: AdjustmentCardParams,
    party_type: CounterpartyType,
    active: &str,
    path: &'static str,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let page = p.page.unwrap_or(1);
    let result = state
        .adjustment_service()
        .list_adjustments(
            &service_ctx,
            &mut conn,
            AdjustmentFilter {
                party_type: Some(party_type),
                keyword: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(page, 10),
        )
        .await?;

    Ok(Html(
        html! {
            div id="fc-card"
                hx-get=(path)
                hx-trigger="arAdjustmentChanged from:body, apAdjustmentChanged from:body, settlementChanged from:body"
                hx-include="#fc-filter-form"
                hx-target="this" hx-select="#fc-card" hx-swap="outerHTML" {
                (fc_tab_bar(active, &summary))
                (adjustments_filter_bar(path, p.keyword.as_deref(), party_type))
                (adjustments_table(&result.items))
                (pagination(path, "#fc-card", "#fc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ⑤ 核销 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SettlementsCardParams {
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("FMS", "read")]
pub async fn get_settlements_card(
    _path: FcSettlementsPath,
    ctx: RequestContext,
    Query(p): Query<SettlementsCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let page = p.page.unwrap_or(1);

    let result = state
        .ar_ap_service()
        .list_settlements(
            &service_ctx,
            &mut conn,
            SettlementFilter::default(),
            PageParams::new(page, 10),
        )
        .await?;

    Ok(Html(
        html! {
            div id="fc-card"
                hx-get=(FcSettlementsPath::PATH)
                hx-trigger="journalChanged from:body, settlementChanged from:body, arAdjustmentChanged from:body, apAdjustmentChanged from:body"
                hx-target="this" hx-select="#fc-card" hx-swap="outerHTML" {
                (fc_tab_bar("settlements", &summary))
                (settlements_table(&result.items))
                (pagination(FcSettlementsPath::PATH, "#fc-card", "#fc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// =============================================================================
// 顶栏 / tab 栏
// =============================================================================

/// detail-header：标题 + 待办总数 + 逾期/临期 pill。
fn fc_summary_header(s: &FmsWorkCenterSummary) -> Markup {
    html! {
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div class="flex items-center gap-2.5" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "财务作业中心" }
                span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-bg text-accent text-xs font-semibold" {
                    span class="font-mono tabular-nums font-bold" { (s.total()) }
                    "待办"
                }
            }
            div class="flex items-center gap-2" {
                @if s.total_overdue() > Decimal::ZERO {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-[11px] font-semibold" {
                        (icon::alert_triangle_icon("w-3 h-3"))
                        "逾期 " (fmt_amount(s.total_overdue()))
                    }
                }
                @if s.total_due_soon() > Decimal::ZERO {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-warn-bg text-warn text-[11px] font-semibold" {
                        (icon::clock_icon("w-3 h-3"))
                        "7天内到期 " (fmt_amount(s.total_due_soon()))
                    }
                }
            }
        }
    }
}

/// tab 标题后的待办计数 badge（>0 才显示），对齐 purchase_work_center::tab_badge。
fn tab_badge(n: u64) -> Markup {
    if n > 0 {
        html! {
            span class="ml-1 inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-accent text-accent-on text-[11px] font-bold font-mono tabular-nums leading-none" {
                (n)
            }
        }
    } else {
        html! {}
    }
}

/// 顶部业务 tab 栏（4 tab + badge）。放进各 tab 端点返回的 HTML，随刷新重渲染。
fn fc_tab_bar(active: &str, s: &FmsWorkCenterSummary) -> Markup {
    let tab = |val: &str,
               path: &str,
               tab_icon: Markup,
               label: &str,
               cnt: u64|
     -> Markup {
        html! {
            button class=(toggle_cls(active == val)) type="button"
                hx-get=(path)
                hx-target="#fc-card" hx-select="#fc-card" hx-swap="outerHTML"
                { (tab_icon) (label) (tab_badge(cnt)) }
        }
    };
    html! {
        div class="flex items-center gap-1 flex-wrap px-5 pt-3 border-b border-border-soft" {
            (tab("receivables", FcReceivablesPath::PATH, icon::trending_up_icon("w-4 h-4"), "应收待收", s.ar_outstanding_count))
            (tab("payables", FcPayablesPath::PATH, icon::payment_icon("w-4 h-4"), "应付待付", s.ap_outstanding_count))
            (tab("ar-adjustments", FcArAdjustmentsPath::PATH, icon::sliders_icon("w-4 h-4"), "应收调整", s.ar_adjustment_total))
            (tab("ap-adjustments", FcApAdjustmentsPath::PATH, icon::sliders_icon("w-4 h-4"), "应付调整", s.ap_adjustment_total))
            (tab("settlements", FcSettlementsPath::PATH, icon::check_circle_icon("w-4 h-4"), "核销", s.settlement_total))
        }
    }
}

fn toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-accent font-semibold cursor-pointer bg-accent-bg rounded-sm border-none transition-colors"
    } else {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-muted font-medium cursor-pointer bg-transparent border-none rounded-sm hover:text-fg hover:bg-surface transition-colors"
    }
}

// =============================================================================
// Drawer overlay
// =============================================================================

/// Drawer overlay 壳（同 purchase_work_center）：背景点击/关闭按钮收起，body 由 `hx-get` 填充。
///
/// 开关：overlay 用 `.drawer-overlay` class，hyperscript toggle `.open`（preflight CSS 驱动显隐+平移）。
fn render_drawer_overlay(overlay_id: &str, _drawer_id: &str, body_id: &str, title: &str, width_class: &str) -> Markup {
    drawer_shell(overlay_id, width_class, html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _=(format!("on click remove .open from #{}", overlay_id)) {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div id=(body_id) class="flex-1 overflow-y-auto px-6 py-5" {}
    })
}

// =============================================================================
// 筛选栏
// =============================================================================

/// 台账 summary 卡片：应收/应付总额、未清、逾期、7 天内到期（复用 ledger_summary）。
fn ledger_summary_cards(s: &LedgerSummary, is_receivable: bool) -> Markup {
    let title_total = if is_receivable { "应收总额" } else { "应付总额" };
    html! {
        div class="grid grid-cols-2 lg:grid-cols-4 gap-3 px-5 py-3 border-b border-border-soft" {
            (summary_card(title_total, &fmt_amount(s.total_amount), "text-fg", icon::dollar_icon("w-4 h-4 text-accent"), "bg-accent-bg"))
            (summary_card("未清余额", &fmt_amount(s.total_outstanding), "text-success", icon::check_circle_icon("w-4 h-4 text-success"), "bg-success-bg"))
            (summary_card("逾期金额", &fmt_amount(s.total_overdue), "text-danger", icon::alert_triangle_icon("w-4 h-4 text-danger"), "bg-danger-bg"))
            (summary_card("7天内到期", &fmt_amount(s.due_within_7d), "text-warn", icon::clock_icon("w-4 h-4 text-warn"), "bg-warn-bg"))
        }
    }
}

/// summary 单卡片（紧凑版，工作中心 tab 内用）。
fn summary_card(title: &str, value: &str, value_cls: &str, icon_svg: Markup, icon_cls: &str) -> Markup {
    html! {
        div class="flex items-center gap-3" {
            div class=({ format!("w-9 h-9 rounded-md grid place-items-center shrink-0 {icon_cls}") }) { (icon_svg) }
            div class="min-w-0" {
                div class="text-xs text-muted" { (title) }
                div class=({ format!("text-lg font-bold font-mono tabular-nums {value_cls}") }) {
                    "¥" (value)
                }
            }
        }
    }
}

/// 应收/应付台账筛选栏：产品/往来方/未清切换 + 可展开高级面板（日期/单号/业务员）+ 导出。
fn ledger_filter_bar(
    path: &str,
    p: &LedgerCardParams,
    outstanding_only: bool,
    buyers: &[String],
    export_type: &str,
) -> Markup {
    let ti = "fc-search-input px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
    let has_filter = p.doc_no.is_some() || p.rep_name.is_some() || p.start_date.is_some() || p.end_date.is_some();
    let panel_cls = if has_filter { "" } else { "hidden" };
    let arrow_cls = if has_filter { "rotate-180" } else { "" };
    let active_cls = "px-2.5 py-1 text-xs rounded-sm bg-accent text-accent-on cursor-pointer";
    let inactive_cls = "px-2.5 py-1 text-xs rounded-sm text-fg-2 hover:bg-surface cursor-pointer";
    html! {
        form id="fc-filter-form" class="px-5 py-2.5 border-b border-border-soft"
            hx-get=(path)
            hx-trigger="change, keyup changed delay:300ms from:.fc-search-input"
            hx-target="#fc-card" hx-select="#fc-card" hx-swap="outerHTML"
            hx-include="#fc-filter-form" {
            input type="hidden" name="filter" value=(if outstanding_only { "outstanding" } else { "all" });
            div class="flex items-center gap-2 flex-wrap" {
                input class=(format!("{} w-36", ti)) type="text" name="product_name" hx-preserve placeholder="产品名称" value=(p.product_name.as_deref().unwrap_or(""));
                input class=(format!("{} flex-1 min-w-[160px] max-w-[220px]", ti)) type="text" name="keyword" hx-preserve placeholder="往来方名称" value=(p.keyword.as_deref().unwrap_or(""));
                input class=(format!("{} w-32", ti)) type="text" name="product_code" hx-preserve placeholder="产品编码" value=(p.product_code.as_deref().unwrap_or(""));
                div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                    a class=(if outstanding_only { active_cls } else { inactive_cls })
                        hx-get=(path) hx-vals=r#"{"filter":"outstanding"}"# hx-target="#fc-card" hx-select="#fc-card" hx-swap="outerHTML" hx-include="#fc-filter-form" { "只看未清" }
                    a class=(if !outstanding_only { active_cls } else { inactive_cls })
                        hx-get=(path) hx-vals=r#"{"filter":"all"}"# hx-target="#fc-card" hx-select="#fc-card" hx-swap="outerHTML" hx-include="#fc-filter-form" { "全部" }
                }
                div class="flex items-center gap-3 ml-auto" {
                    button type="button" class="inline-flex items-center gap-1 text-xs text-fg-2 hover:text-accent cursor-pointer border-none bg-transparent p-0"
                        _="on click toggle .hidden on #fc-filter-panel then toggle .rotate-180 on .filter-arrow" {
                        "高级筛选 "
                        span class=(format!("filter-arrow inline-block transition-transform {arrow_cls}")) { "▾" }
                    }
                    (crate::components::export_button::export_button("导出", export_type, Some("#fc-filter-form")))
                }
            }
            div id="fc-filter-panel" class=(format!("{} flex items-center gap-3 pt-3 mt-3 border-t border-border-soft flex-wrap", panel_cls)) {
                span class="text-xs text-fg-2" { "发生日期" }
                input class=(ti) type="date" name="start_date" hx-preserve value=(p.start_date.as_deref().unwrap_or(""));
                span class="text-fg-3 text-xs" { "至" }
                input class=(ti) type="date" name="end_date" hx-preserve value=(p.end_date.as_deref().unwrap_or(""));
                span class="text-fg-3 mx-1" { "|" }
                span class="text-xs text-fg-2" { "发生单号" }
                input class=(ti) type="text" name="doc_no" hx-preserve placeholder="模糊搜索" value=(p.doc_no.as_deref().unwrap_or(""));
                span class="text-fg-3 mx-1" { "|" }
                span class="text-xs text-fg-2" { "业务员" }
                select class=(ti) name="rep_name" hx-preserve {
                    option value="" { "全部" }
                    @for name in buyers {
                        option value=(name) selected[(p.rep_name.as_deref() == Some(name.as_str()))] { (name) }
                    }
                }
            }
        }
    }
}

/// 应收/应付调整筛选栏：往来方搜索 + 新建调整 drawer 入口。
fn adjustments_filter_bar(path: &str, keyword: Option<&str>, party_type: CounterpartyType) -> Markup {
    let kw = keyword.unwrap_or("");
    let cp_val: i16 = match party_type {
        CounterpartyType::Customer => 1,
        _ => 2,
    };
    let drawer_url = FcAdjustmentDrawerPath { party_type: cp_val }.to_string();
    html! {
        div class="flex items-center gap-2 px-5 py-2.5 border-b border-border-soft flex-wrap" {
            form id="fc-filter-form" class="flex items-center gap-2 flex-1"
                hx-get=(path)
                hx-trigger="change, keyup changed delay:300ms from:.fc-search-input"
                hx-target="#fc-card" hx-select="#fc-card" hx-swap="outerHTML"
                hx-include="#fc-filter-form" {
                input type="text" name="keyword" value=(kw) placeholder="搜往来方名称"
                    class="fc-search-input flex-1 max-w-[260px] px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
            }
            button type="button"
                class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-sm bg-accent text-accent-on text-xs font-medium border-none cursor-pointer hover:bg-accent-hover shrink-0"
                hx-get=(drawer_url)
                hx-target="#fc-adjustment-drawer-body" hx-swap="innerHTML"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-adjustment-overlay" {
                (icon::plus_icon("w-3.5 h-3.5")) "新建调整"
            }
        }
    }
}

// =============================================================================
// 表格渲染
// =============================================================================

/// 应收/应付台账表格（AR/AP 共用）。列：到期日 / 往来方 / 单据号 / 产品 / 未清余额 / 状态 / 操作。
fn ledger_table(rows: &[ArApLedgerRow], today: NaiveDate, is_receivable: bool) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="data-table w-full" {
                thead {
                    tr {
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "到期日" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "往来方" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "发生单号" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "产品" }
                        th class="py-2.5 px-4 text-right font-semibold text-muted text-xs uppercase tracking-wide" { "未清余额" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "状态" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if rows.is_empty() {
                        tr { td colspan="7" class="py-10 text-center text-muted text-sm" { "暂无待办记录" } }
                    } @else {
                        @for r in rows {
                            tr class="hover:bg-surface" {
                                td class="py-2.5 px-4 text-sm text-fg-2 font-mono" { (fmt_date_opt(&r.due_date)) }
                                td class="py-2.5 px-4 text-sm text-fg" {
                                    a class="text-fg hover:text-accent hover:underline cursor-pointer"
                                        href=(if is_receivable {
                                            crate::routes::customer::CustomerDetailPath { id: r.party_id }.to_string()
                                        } else {
                                            crate::routes::supplier::SupplierDetailPath { id: r.party_id }.to_string()
                                        })
                                        target="_blank" { (r.party_name) }
                                }
                                td class="py-2.5 px-4 text-sm font-mono" {
                                    button type="button" class="text-fg-2 text-sm font-mono bg-transparent border-none p-0 cursor-pointer hover:text-accent hover:underline text-left"
                                        hx-get=(FcLedgerDetailDrawerPath { id: r.id }.to_string())
                                        hx-target="#fc-ledger-detail-drawer-body" hx-swap="innerHTML"
                                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-ledger-detail-overlay" { (r.source_doc_no) }
                                }
                                td class="py-2.5 px-4 text-sm text-muted max-w-[200px] truncate" { (r.product_summary.as_deref().unwrap_or("—")) }
                                td class="py-2.5 px-4 text-sm text-right text-fg font-mono tabular-nums" { (fmt_amount(r.amount_outstanding)) (currency_tag(&r.currency)) }
                                td class="py-2.5 px-4 text-sm" { (ledger_row_status(&r.due_date, r.amount_outstanding, today)) }
                                td class="py-2.5 px-4 text-sm" {
                                    div class="flex items-center gap-1.5" {
                                        @if r.amount_outstanding > Decimal::ZERO {
                                            @if is_receivable {
                                                button type="button"
                                                    class="inline-flex items-center px-2.5 py-1 rounded-sm bg-success-bg text-success text-xs font-medium border-none cursor-pointer hover:opacity-80"
                                                    hx-get=(FcReceiptDrawerPath { id: r.id }.to_string())
                                                    hx-target="#fc-receipt-drawer-body" hx-swap="innerHTML"
                                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-receipt-overlay" {
                                                    "登记收款"
                                                }
                                            } @else {
                                                button type="button"
                                                    class="inline-flex items-center px-2.5 py-1 rounded-sm bg-warn-bg text-warn text-xs font-medium border-none cursor-pointer hover:opacity-80"
                                                    hx-get=(FcPaymentDrawerPath { id: r.id }.to_string())
                                                    hx-target="#fc-payment-drawer-body" hx-swap="innerHTML"
                                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-payment-overlay" {
                                                    "登记付款"
                                                }
                                            }
                                            button type="button"
                                                class="inline-flex items-center px-2.5 py-1 rounded-sm bg-purple-bg text-purple text-xs font-medium border-none cursor-pointer hover:opacity-80"
                                                hx-get=(FcSettleDrawerPath { party_type: if is_receivable { 1 } else { 2 }, party_id: r.party_id }.to_string())
                                                hx-target="#fc-settle-drawer-body" hx-swap="innerHTML"
                                                _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-settle-overlay" {
                                                "核销"
                                            }
                                        }
                                        button type="button"
                                            class="inline-flex items-center px-2 py-1 rounded-sm bg-surface text-muted text-xs font-medium border border-border-soft cursor-pointer hover:bg-surface-raised"
                                            hx-get=(FcLedgerDetailDrawerPath { id: r.id }.to_string())
                                            hx-target="#fc-ledger-detail-drawer-body" hx-swap="innerHTML"
                                            _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-ledger-detail-overlay" {
                                            "明细"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 应收/应付调整表格。列：调整日期 / 调整单号 / 往来方 / 方向 / 金额 / 说明。
fn adjustments_table(rows: &[AdjustmentRow]) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="data-table w-full" {
                thead {
                    tr {
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "调整日期" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "调整单号" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "往来方" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "方向" }
                        th class="py-2.5 px-4 text-right font-semibold text-muted text-xs uppercase tracking-wide" { "金额" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "内部订单号" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "外部订单号" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "说明" }
                    }
                }
                tbody {
                    @if rows.is_empty() {
                        tr { td colspan="8" class="py-10 text-center text-muted text-sm" { "暂无调整记录" } }
                    } @else {
                        @for a in rows {
                            tr class="hover:bg-surface" {
                                td class="py-2.5 px-4 text-sm text-fg-2 font-mono" { (fmt_date(a.adjustment_date)) }
                                td class="py-2.5 px-4 text-sm font-mono" {
                                    button type="button" class="text-fg text-sm font-mono bg-transparent border-none p-0 cursor-pointer hover:text-accent hover:underline text-left"
                                        hx-get=(FcAdjustmentDetailDrawerPath { id: a.id }.to_string())
                                        hx-target="#fc-adjustment-detail-drawer-body" hx-swap="innerHTML"
                                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #fc-adjustment-detail-overlay" { (a.doc_number) }
                                }
                                td class="py-2.5 px-4 text-sm text-fg" {
                                    a class="text-fg hover:text-accent hover:underline cursor-pointer"
                                        href=(match a.party_type {
                                            CounterpartyType::Customer => crate::routes::customer::CustomerDetailPath { id: a.party_id }.to_string(),
                                            CounterpartyType::Supplier => crate::routes::supplier::SupplierDetailPath { id: a.party_id }.to_string(),
                                            _ => "#".to_string(),
                                        })
                                        target="_blank" { (a.party_name) }
                                }
                                td class="py-2.5 px-4 text-sm" { (adjustment_direction_badge(&a.direction)) }
                                td class="py-2.5 px-4 text-sm text-right text-fg font-mono tabular-nums" { (fmt_amount(a.amount)) (currency_tag(&a.currency)) }
                                td class="py-2.5 px-4 text-sm text-fg-2" { (a.int_order_no.as_deref().unwrap_or("—")) }
                                td class="py-2.5 px-4 text-sm text-fg-2" { (a.ext_order_no.as_deref().unwrap_or("—")) }
                                td class="py-2.5 px-4 text-sm text-muted" { (a.description) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 核销记录表格。列：日期 / 付款单 / 发票单 / 核销金额 / 汇兑损益。
fn settlements_table(rows: &[ArApSettlement]) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="data-table w-full" {
                thead {
                    tr {
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "核销日期" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "付款来源" }
                        th class="py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide" { "发票来源" }
                        th class="py-2.5 px-4 text-right font-semibold text-muted text-xs uppercase tracking-wide" { "核销金额" }
                        th class="py-2.5 px-4 text-right font-semibold text-muted text-xs uppercase tracking-wide" { "汇兑损益" }
                        th class="py-2.5 px-4 text-right font-semibold text-muted text-xs uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if rows.is_empty() {
                        tr { td colspan="6" class="py-10 text-center text-muted text-sm" { "暂无核销记录" } }
                    } @else {
                        @for s in rows {
                            tr class="hover:bg-surface" {
                                td class="py-2.5 px-4 text-sm text-fg-2 font-mono" { (fmt_date(s.settlement_date)) }
                                td class="py-2.5 px-4 text-sm text-fg-2 font-mono" { (doc_type_label(&s.payment_source_type)) "#" (s.payment_source_id) }
                                td class="py-2.5 px-4 text-sm text-fg-2 font-mono" { (doc_type_label(&s.invoice_source_type)) "#" (s.invoice_source_id) }
                                td class="py-2.5 px-4 text-sm text-right text-fg font-mono tabular-nums" { (fmt_amount(s.amount)) }
                                td class="py-2.5 px-4 text-sm text-right text-muted font-mono tabular-nums" { (fmt_amount(s.exchange_gain_loss)) }
                                td class="py-2.5 px-4 text-sm text-right" {
                                    form hx-post=({ FcSettlementUnsettlePath { id: s.id }.to_string() })
                                          hx-confirm="确认撤销此核销？" hx-swap="none" {
                                        button type="submit" class="text-danger text-xs hover:underline cursor-pointer" { "撤销" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// =============================================================================
// Drawer 端点 + 写操作（Phase 2）
// =============================================================================

// ── 确认收付款 drawer ──

// ── 登记收款 / 登记付款 drawer ──

#[require_permission("FMS", "read")]
pub async fn get_receipt_drawer(
    path: FcReceiptDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let (row, _) = state
        .ar_ap_service()
        .get_ledger_detail(&service_ctx, &mut conn, path.id)
        .await?
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("台账记录"))?;
    Ok(Html(render_cash_drawer_body(&row, true).into_string()))
}

#[require_permission("FMS", "read")]
pub async fn get_payment_drawer(
    path: FcPaymentDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let (row, _) = state
        .ar_ap_service()
        .get_ledger_detail(&service_ctx, &mut conn, path.id)
        .await?
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("台账记录"))?;
    Ok(Html(render_cash_drawer_body(&row, false).into_string()))
}

fn render_cash_drawer_body(row: &ArApLedgerRow, is_receivable: bool) -> Markup {
    let action = if is_receivable { "收款" } else { "付款" };
    let post_path = if is_receivable {
        FcJournalReceiptPath { id: row.id }.to_string()
    } else {
        FcJournalPaymentPath { id: row.id }.to_string()
    };
    let overlay_id = if is_receivable {
        "fc-receipt-overlay"
    } else {
        "fc-payment-overlay"
    };
    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
    html! {
        form class="space-y-3"
            hx-post=(post_path)
            hx-target="this" hx-swap="none"
            _=(format!("on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #{} then call showToast('已登记{}')", overlay_id, action)) {
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "往来方" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg" { (row.party_name) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "发生单号" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (row.source_doc_no) }
                }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "未清余额" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg font-mono tabular-nums" {
                        (fmt_amount(row.amount_outstanding)) (currency_tag(&row.currency))
                    }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "本次" (action) "金额" }
                    input type="text" name="amount" value=(fmt_amount(row.amount_outstanding))
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono tabular-nums"
                        required;
                }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "日期" }
                    input type="date" name="transaction_date" value=(today)
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "银行账户" }
                    input type="text" name="bank_account" placeholder="银行账号"
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
                }
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                textarea name="remark" rows="2"
                    class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y" {}
            }
            div class="p-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
                "提交后自动创建" (action) "凭证并过账：立反向 AR/AP 台账；源单为发货/入库时自动核销对应往来款。"
            }
            div class="flex justify-end gap-2 pt-3 border-t border-border-soft" {
                button type="button" class="inline-flex items-center px-4 py-2 rounded-sm bg-white text-muted border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    _=(format!("on click remove .open from #{}", overlay_id)) {
                    "取消"
                }
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "确认" (action)
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CashForm {
    pub amount: String,
    pub transaction_date: String,
    #[serde(default)]
    pub bank_account: String,
    #[serde(default)]
    pub remark: String,
}

#[require_permission("FMS", "create")]
pub async fn create_receipt(
    path: FcJournalReceiptPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CashForm>,
) -> Result<impl IntoResponse> {
    do_create_cash(ctx, form, path.id, true).await?;
    Ok(([("HX-Trigger", "journalChanged")], Html(String::new())))
}

#[require_permission("FMS", "create")]
pub async fn create_payment(
    path: FcJournalPaymentPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CashForm>,
) -> Result<impl IntoResponse> {
    do_create_cash(ctx, form, path.id, false).await?;
    Ok(([("HX-Trigger", "journalChanged")], Html(String::new())))
}

/// 登记收/付款核心逻辑：查台账行 → 构造 CashJournal（SalesReceipt/PurchasePayment）→ create + confirm → 事务提交。
async fn do_create_cash(
    ctx: RequestContext,
    form: CashForm,
    ledger_id: i64,
    is_receivable: bool,
) -> Result<()> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let date = NaiveDate::parse_from_str(&form.transaction_date, "%Y-%m-%d")
        .map_err(|e| abt_core::shared::types::DomainError::validation(format!("日期格式错误: {e}")))?;
    let amount = form
        .amount
        .parse::<Decimal>()
        .map_err(|e| abt_core::shared::types::DomainError::validation(format!("金额格式错误: {e}")))?;
    if amount <= Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::validation("金额必须大于 0").into());
    }
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let (row, _) = state
        .ar_ap_service()
        .get_ledger_detail(&service_ctx, &mut tx, ledger_id)
        .await?
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("台账记录"))?;
    let counterparty = if is_receivable {
        abt_core::fms::enums::CounterpartyRef::Customer(row.party_id)
    } else {
        abt_core::fms::enums::CounterpartyRef::Supplier(row.party_id)
    };
    let req = CreateCashJournalReq {
        journal_type: if is_receivable {
            JournalType::SalesReceipt
        } else {
            JournalType::PurchasePayment
        },
        direction: if is_receivable {
            CashDirection::Inflow
        } else {
            CashDirection::Outflow
        },
        amount,
        counterparty,
        source_type: row.source_type,
        source_id: row.source_id,
        bank_account: form.bank_account,
        transaction_date: date,
        period: date.format("%Y-%m").to_string(),
        remark: form.remark,
        currency: row.currency.clone(),
        exchange_rate: Decimal::ONE,
        lines: vec![],
    };
    let cj_id = state
        .cash_journal_service()
        .create(&service_ctx, &mut tx, req)
        .await?;
    state
        .cash_journal_service()
        .confirm(&service_ctx, &mut tx, cj_id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    invalidate_fms_summary(&state);
    Ok(())
}

// ── 台账明细 drawer（行展开）──

#[require_permission("FMS", "read")]
pub async fn get_ledger_detail_drawer(
    path: FcLedgerDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let detail = state
        .ar_ap_service()
        .get_ledger_detail(&service_ctx, &mut conn, path.id)
        .await?;
    Ok(Html(
        match detail {
            Some((row, items)) => render_ledger_detail_body(&row, &items).into_string(),
            None => html! {
                div class="text-muted text-sm p-4" { "未找到台账记录" }
            }
            .into_string(),
        },
    ))
}

/// 调整单详情 drawer body（只读）：单号+方向 / 往来方 / 金额+币种 / 日期+期间 / 内外部订单号 / 说明 / 操作人+时间。
#[require_permission("FMS", "read")]
pub async fn get_adjustment_detail_drawer(
    path: FcAdjustmentDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let adj = state
        .adjustment_service()
        .get_adjustment(&service_ctx, &mut conn, path.id)
        .await?;
    let party_name = match adj.party_type {
        CounterpartyType::Customer => state
            .customer_service()
            .get(&service_ctx, &mut conn, adj.party_id)
            .await
            .map(|c| c.name)
            .unwrap_or_else(|_| format!("客户 #{}", adj.party_id)),
        CounterpartyType::Supplier => state
            .supplier_service()
            .get(&service_ctx, &mut conn, adj.party_id)
            .await
            .map(|s| s.name)
            .unwrap_or_else(|_| format!("供应商 #{}", adj.party_id)),
        _ => format!("往来方 #{}", adj.party_id),
    };
    let operator_name = state
        .user_service()
        .get_user(&service_ctx, &mut conn, adj.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());
    Ok(Html(
        render_adjustment_detail_body(&adj, &party_name, &operator_name).into_string(),
    ))
}

fn render_adjustment_detail_body(adj: &ArApAdjustment, party_name: &str, operator_name: &str) -> Markup {
    html! {
        div class="space-y-3" {
            div class="flex items-center gap-2.5 mb-4" {
                span class="text-base font-bold font-mono text-fg" { (adj.doc_number) }
                (adjustment_direction_badge(&adj.direction))
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "往来方" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg" { (party_name) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "金额" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg font-mono" { (fmt_amount(adj.amount)) (currency_tag(&adj.currency)) }
                }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "调整日期" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (adj.adjustment_date.format("%Y-%m-%d")) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "期间" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (&adj.period) }
                }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "内部订单号" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (adj.int_order_no.as_deref().unwrap_or("—")) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "外部订单号" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (adj.ext_order_no.as_deref().unwrap_or("—")) }
                }
            }
            @if adj.currency != "CNY" {
                div class="grid grid-cols-2 gap-4" {
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" { "币种" }
                        div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (&adj.currency) }
                    }
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" { "汇率" }
                        div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (adj.exchange_rate) }
                    }
                }
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "说明" }
                div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2" { (if adj.description.is_empty() { "—" } else { adj.description.as_str() }) }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "操作人" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2" { (operator_name) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "创建时间" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (adj.created_at.format("%Y-%m-%d %H:%M")) }
                }
            }
        }
    }
}

fn render_ledger_detail_body(row: &ArApLedgerRow, items: &[LedgerDetailItem]) -> Markup {
    let today = chrono::Utc::now().date_naive();
    html! {
        div class="space-y-3" {
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "往来方" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg" { (row.party_name) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "发生单号" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (row.source_doc_no) }
                }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "销售单号" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (row.upstream_doc_no.as_deref().unwrap_or("—")) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "发生日期" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (row.transaction_date.format("%Y-%m-%d")) }
                }
            }
            div class="grid grid-cols-3 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "金额" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg font-mono" { (fmt_amount(row.amount)) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "已核销" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (fmt_amount(row.amount_applied)) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "未清余额" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-danger font-mono" { (fmt_amount(row.amount_outstanding)) }
                }
            }
            div class="grid grid-cols-2 gap-4" {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "到期日" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (fmt_date_opt(&row.due_date)) }
                }
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "状态" }
                    div class="px-2.5 py-1.5" { (ledger_row_status(&row.due_date, row.amount_outstanding, today)) }
                }
            }
            @if !row.description.is_empty() {
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "说明" }
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2" { (row.description) }
                }
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "产品明细" }
                @if items.is_empty() {
                    div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-muted" { "无产品明细" }
                } @else {
                    div class="overflow-x-auto" {
                        table class="data-table w-full text-sm" {
                            thead {
                                tr {
                                    th class="px-3 py-2 text-left text-xs font-medium text-fg-2 uppercase" { "编码" }
                                    th class="px-3 py-2 text-left text-xs font-medium text-fg-2 uppercase" { "名称" }
                                    th class="px-3 py-2 text-right text-xs font-medium text-fg-2 uppercase" { "数量" }
                                    th class="px-3 py-2 text-right text-xs font-medium text-fg-2 uppercase" { "单价" }
                                    th class="px-3 py-2 text-right text-xs font-medium text-fg-2 uppercase" { "行金额" }
                                }
                            }
                            tbody class="divide-y divide-border-soft" {
                                @for item in items {
                                    tr {
                                        td class="px-3 py-2 font-mono text-xs" { (item.product_code) }
                                        td class="px-3 py-2 text-xs" { (item.product_name) }
                                        td class="px-3 py-2 text-right font-mono text-xs tabular-nums" { (fmt_amount(item.quantity)) }
                                        td class="px-3 py-2 text-right font-mono text-xs tabular-nums" { "¥" (fmt_amount(item.unit_price)) }
                                        td class="px-3 py-2 text-right font-mono text-xs tabular-nums font-semibold" { "¥" (fmt_amount(item.line_amount)) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// =============================================================================
// 手动核销 drawer（Phase 2c）
// =============================================================================

#[require_permission("FMS", "read")]
pub async fn get_settle_drawer(
    path: FcSettleDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let party_type = CounterpartyType::from_i16(path.party_type)
        .ok_or_else(|| {
            abt_core::shared::types::DomainError::validation(format!(
                "无效往来方类型: {}",
                path.party_type
            ))
        })?;
    let invoices = state
        .ar_ap_service()
        .list_open_invoices(&service_ctx, &mut conn, party_type, path.party_id)
        .await?;
    let payments = state
        .ar_ap_service()
        .list_unapplied_payments(&service_ctx, &mut conn, party_type, path.party_id)
        .await?;
    Ok(Html(
        render_settle_drawer_body(party_type, path.party_id, &invoices, &payments).into_string(),
    ))
}

fn render_settle_drawer_body(
    party_type: CounterpartyType,
    party_id: i64,
    invoices: &[OpenInvoice],
    payments: &[UnappliedPayment],
) -> Markup {
    let party_label = match party_type {
        CounterpartyType::Customer => "客户",
        CounterpartyType::Supplier => "供应商",
        CounterpartyType::Employee => "员工",
        CounterpartyType::Other => "其他",
    };
    html! {
        form class="space-y-3"
            hx-post=(FcSettlePath::PATH)
            hx-target="this" hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #fc-settle-overlay then call showToast('已核销')" {
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "往来方" }
                div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg" {
                    (party_label) " #" (party_id)
                }
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "选择应收/应付单（未清）" }
                select name="invoice" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {
                    @if invoices.is_empty() {
                        option value="" disabled selected { "（无未清单据）" }
                    } @else {
                        @for inv in invoices {
                            option value=(format!("{}:{}", inv.source_type.as_i16(), inv.source_id)) {
                                (inv.doc_number) " · 余额 " (fmt_amount(inv.outstanding)) " " (inv.currency)
                            }
                        }
                    }
                }
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "选择收/付款（未分配）" }
                select name="payment" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {
                    @if payments.is_empty() {
                        option value="" disabled selected { "（无未分配收/付款）" }
                    } @else {
                        @for pay in payments {
                            option value=(format!("{}:{}", pay.source_type.as_i16(), pay.source_id)) {
                                (pay.doc_number) " · 未分配 " (fmt_amount(pay.unapplied)) " " (pay.currency)
                            }
                        }
                    }
                }
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "核销金额" }
                input type="text" name="amount" placeholder="0.00"
                    class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono tabular-nums" required;
            }
            div class="p-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
                "核销将双向更新台账 amount_applied；金额不得超过所选单据的未清余额与收/付款的未分配额。"
            }
            div class="flex justify-end gap-2 pt-3 border-t border-border-soft" {
                button type="button" class="inline-flex items-center px-4 py-2 rounded-sm bg-white text-muted border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    _="on click remove .open from #fc-settle-overlay" {
                    "取消"
                }
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "确认核销"
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SettleForm {
    /// "source_type:source_id"
    pub invoice: String,
    /// "source_type:source_id"
    pub payment: String,
    pub amount: String,
}

/// 解析 "type:id" → (DocumentType, i64)
fn parse_type_id(
    s: &str,
) -> std::result::Result<(DocumentType, i64), abt_core::shared::types::DomainError> {
    let (t, i) = s
        .split_once(':')
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("单据格式错误"))?;
    let doc_type = t
        .parse::<i16>()
        .ok()
        .and_then(DocumentType::from_i16)
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效单据类型"))?;
    let id = i
        .parse::<i64>()
        .map_err(|_| abt_core::shared::types::DomainError::validation("无效单据 ID"))?;
    Ok((doc_type, id))
}

#[require_permission("FMS", "update")]
pub async fn settle(
    _path: FcSettlePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SettleForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let (inv_type, inv_id) = parse_type_id(&form.invoice)?;
    let (pay_type, pay_id) = parse_type_id(&form.payment)?;
    let amount = form
        .amount
        .parse::<Decimal>()
        .map_err(|e| abt_core::shared::types::DomainError::validation(format!("金额格式错误: {e}")))?;
    if amount <= Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::validation("核销金额必须大于 0").into());
    }
    let req = SettleReq {
        payment_source_type: pay_type,
        payment_source_id: pay_id,
        invoice_source_type: inv_type,
        invoice_source_id: inv_id,
        amount,
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    state.ar_ap_service().settle(&service_ctx, &mut tx, req).await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    invalidate_fms_summary(&state);
    Ok(([("HX-Trigger", "settlementChanged")], Html(String::new())))
}

/// 撤销核销（从原 fms_settlement 页迁入）。事务 + 广播 settlementChanged，#fc-card 自刷新。
#[require_permission("FMS", "update")]
pub async fn unsettle(
    _path: FcSettlementUnsettlePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    state
        .ar_ap_service()
        .unsettle(&service_ctx, &mut tx, _path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    invalidate_fms_summary(&state);
    Ok(([("HX-Trigger", "settlementChanged")], Html(String::new())))
}

// =============================================================================
// 调整创建 drawer（#190：a href 改 drawer 就地操作）
// =============================================================================

/// GET：加载调整创建表单到 drawer body。
#[require_permission("FMS", "read")]
pub async fn get_adjustment_drawer(
    path: FcAdjustmentDrawerPath,
    _ctx: RequestContext,
) -> Result<Html<String>> {
    let party_type = CounterpartyType::from_i16(path.party_type)
        .unwrap_or(CounterpartyType::Customer);
    Ok(Html(render_adjustment_drawer_body(party_type).into_string()))
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdjCreateForm {
    pub counterparty_type: i16,
    pub party_id: i64,
    pub direction: i16,
    pub amount: String,
    pub currency: String,
    pub exchange_rate: String,
    pub adjustment_date: String,
    #[serde(default)]
    pub int_order_no: Option<String>,
    #[serde(default)]
    pub ext_order_no: Option<String>,
    #[serde(default)]
    pub description: String,
}

/// POST：创建调整（事务 + 广播事件 + 空 body 关 drawer）。
#[require_permission("FMS", "create")]
pub async fn create_adjustment(
    _path: FcAdjustmentCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<AdjCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;

    let party_type = CounterpartyType::from_i16(form.counterparty_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;
    if form.party_id <= 0 {
        return Err(abt_core::shared::types::DomainError::Validation("请选择往来方".into()).into());
    }
    let direction = AdjustmentDirection::from_i16(form.direction)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效调整方向".into()))?;
    let amount = rust_decimal::Decimal::from_str_exact(&form.amount)
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效金额".into()))?;
    let adjustment_date = chrono::NaiveDate::parse_from_str(&form.adjustment_date, "%Y-%m-%d")
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效调整日期".into()))?;
    let period = form
        .adjustment_date
        .get(..7)
        .unwrap_or(&form.adjustment_date)
        .to_string();

    let currency = form.currency.trim().to_uppercase();
    let exchange_rate = if currency == "CNY" {
        rust_decimal::Decimal::ONE
    } else {
        rust_decimal::Decimal::from_str_exact(form.exchange_rate.trim())
            .map_err(|_| abt_core::shared::types::DomainError::Validation("无效汇率".into()))?
    };

    let req = CreateAdjustmentReq {
        party_type,
        party_id: form.party_id,
        direction,
        amount,
        adjustment_date,
        period,
        int_order_no: form.int_order_no.filter(|s| !s.is_empty()),
        ext_order_no: form.ext_order_no.filter(|s| !s.is_empty()),
        description: form.description,
        currency,
        exchange_rate,
    };

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    state
        .adjustment_service()
        .create_adjustment(&service_ctx, &mut tx, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    let event = if party_type == CounterpartyType::Customer {
        "arAdjustmentChanged"
    } else {
        "apAdjustmentChanged"
    };
    invalidate_fms_summary(&state);
    Ok(([("HX-Trigger", event)], Html(String::new())))
}

/// 调整创建表单 body（照搬 fms_adjustment_create.rs 表单，去掉 admin_page 壳）。
fn render_adjustment_drawer_body(party_type: CounterpartyType) -> Markup {
    let (cp_label, picker_title, placeholder, balance_label) = match party_type {
        CounterpartyType::Customer => ("客户", "选择客户", "搜索选择客户…", "应收金额"),
        _ => ("供应商", "选择供应商", "搜索选择供应商…", "应付金额"),
    };
    let cp_type_val: i16 = match party_type {
        CounterpartyType::Customer => 1,
        _ => 2,
    };

    let cp_picker = EntityPickerConfig {
        modal_id: "fc-adj-cp-picker",
        title: picker_title,
        search_label: "关键词",
        search_placeholder: "搜索名称或编码…",
        search_path: JournalSearchCpPath::PATH,
        search_param: "q",
        target_id: "fc-adj-party-id",
        display_id: "fc-adj-party-display",
        event_name: "fcAdjCpSelected",
        extra_include: Some("#fc-adj-cp-type"),
    };

    html! {
        div class="space-y-3" {
            form id="fc-adjustment-create-form"
                hx-post=(FcAdjustmentCreatePath::PATH)
                hx-target="this" hx-swap="none"
                _=(format!("on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #fc-adjustment-overlay then call showToast('已创建{adj_label}调整')", adj_label = if party_type == CounterpartyType::Customer { "应收" } else { "应付" })) {
                input type="hidden" id="fc-adj-cp-type" name="counterparty_type" value=(cp_type_val);

                // 往来方 + 余额
                div class="grid grid-cols-2 gap-4" {
                    div {
                        label class="block text-xs font-medium text-fg-2 mb-1" {
                            (cp_label) " " span class="text-danger" { "*" }
                        }
                        (entity_picker::entity_picker_field(
                            "party_id", "fc-adj-party-id", "fc-adj-party-display", "fc-adj-cp-picker",
                            &format!("{cp_label} "), true, placeholder,
                        ))
                    }
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" { (balance_label) }
                        div id="fc-adj-balance"
                            class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg font-mono tabular-nums"
                            hx-get=(FcAdjustmentBalancePath::PATH)
                            hx-trigger="fcAdjCpSelected from:body"
                            hx-target="this"
                            hx-swap="innerHTML"
                            hx-include="#fc-adj-party-id"
                            hx-vals=(format!("{{\"party_type\":\"{cp_type_val}\"}}")) {
                            div class="text-sm text-muted" { "请先选择" (cp_label) }
                        }
                    }
                }
                // 方向 + 金额 + 币种
                div class="grid grid-cols-2 gap-4" {
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" {
                            "调整方向 " span class="text-danger" { "*" }
                        }
                        select name="direction" required
                            class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {
                            option value="" disabled selected { "请选择" }
                            option value="1" { "增加" }
                            option value="2" { "减少" }
                        }
                    }
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" {
                            "调整金额(含税) " span class="text-danger" { "*" }
                        }
                        div class="flex gap-2" {
                            input type="number" step="any" name="amount" id="fc-adj-amount" required
                                class="flex-1 min-w-0 px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono text-right"
                                placeholder="0.00" _="on input call fcAdjCalcCny()";
                            select name="currency" id="fc-adj-currency"
                                class="!w-24 shrink-0 px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                                _="on change call fcAdjUpdateRate() then call fcAdjCalcCny()" {
                                option value="CNY" selected { "CNY ¥" }
                                option value="USD" { "USD $" }
                                option value="EUR" { "EUR €" }
                                option value="HKD" { "HKD HK$" }
                            }
                        }
                    }
                }
                // 汇率 + 折合人民币 + 调整日期 + 调整单号
                div class="grid grid-cols-2 gap-4" {
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" { "汇率" }
                        input type="number" step="any" min="0" name="exchange_rate" id="fc-adj-exg-rate"
                            value="1" readonly
                            class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-surface text-fg-2 font-mono text-right outline-none"
                            _="on input call fcAdjCalcCny()";
                    }
                    div id="fc-adj-amount-cny"
                        class="flex-1 min-w-0 px-2.5 py-1.5 border border-border-soft rounded-sm text-sm bg-surface text-fg font-mono text-right flex items-center" {
                        "¥0.00"
                    }
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" {
                            "调整日期 " span class="text-danger" { "*" }
                        }
                        input type="date" name="adjustment_date" required
                            class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
                    }
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" { "调整单号" }
                        input type="text" disabled value="系统自动生成"
                            class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-surface text-muted cursor-not-allowed";
                    }
                }
                // 内部订单号 + 外部订单号
                div class="grid grid-cols-2 gap-4" {
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" { "内部订单号" }
                        input type="text" name="int_order_no"
                            class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                            placeholder="可选";
                    }
                    div class="mb-4" {
                        label class="block text-xs text-muted font-medium mb-1.5" {
                            (cp_label) "订单号"
                        }
                        input type="text" name="ext_order_no"
                            class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                            placeholder="可选";
                    }
                }
                // 简要说明
                div class="mb-4" {
                    label class="block text-xs text-muted font-medium mb-1.5" { "简要说明" }
                    textarea name="description" rows="2"
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y"
                        placeholder="如：坏账核销 / 折扣 / 抹零 / 错误更正…" {}
                }
            }
            (entity_picker::entity_picker_modal(&cp_picker))
            // 底部操作栏
            div class="flex justify-end gap-2 pt-3 border-t border-border-soft" {
                button type="button"
                    class="inline-flex items-center px-4 py-2 rounded-sm bg-white text-muted border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    _="on click remove .open from #fc-adjustment-overlay" {
                    "取消"
                }
                button type="button"
                    class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90"
                    _="on click trigger submit on #fc-adjustment-create-form" {
                    "提交"
                }
            }
            // ── 多币种折算 JS ──
            (PreEscaped(r#"<script>
function fcAdjUpdateRate(){var c=document.getElementById('fc-adj-currency').value;var r=document.getElementById('fc-adj-exg-rate');if(!r)return;if(c==='CNY'){r.value='1';r.readOnly=true;r.classList.add('bg-surface','text-fg-2');r.classList.remove('bg-white','text-fg');}else{r.readOnly=false;r.classList.remove('bg-surface','text-fg-2');r.classList.add('bg-white','text-fg');}}
function fcAdjCalcCny(){var a=parseFloat(document.getElementById('fc-adj-amount').value)||0;var r=parseFloat(document.getElementById('fc-adj-exg-rate').value)||0;var b=document.getElementById('fc-adj-amount-cny');if(b)b.textContent='¥'+(a*r).toFixed(2);}
fcAdjUpdateRate();fcAdjCalcCny();
</script>"#))
        }
    }
}

// =============================================================================
// 调整 drawer — 余额查询
// =============================================================================

/// 选往来方后 htmx 加载该方余额（从 fms_adjustment_create 迁入）。
#[derive(Debug, Deserialize)]
pub(crate) struct AdjBalanceQuery {
    pub party_type: i16,
    pub party_id: i64,
}

#[require_permission("FMS", "read")]
pub async fn get_adjustment_balance(
    _path: FcAdjustmentBalancePath,
    ctx: RequestContext,
    Query(q): Query<AdjBalanceQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let party_type = CounterpartyType::from_i16(q.party_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;
    let balance = state.ar_ap_service()
        .get_party_balance(&service_ctx, &mut conn, party_type, q.party_id).await.ok();
    let (amount, label) = match (&balance, party_type) {
        (Some(b), CounterpartyType::Customer) => (b.total_ar, "当前应收余额"),
        (Some(b), _) => (b.total_ap, "当前应付余额"),
        _ => return Ok(Html("<div class='text-sm text-muted'>无法获取余额</div>".to_string())),
    };
    Ok(Html(format!("<div class='text-2xl font-bold font-mono tabular-nums text-fg'>¥{amount:.2}</div><div class='text-xs text-muted mt-1'>{label}</div>")))
}

// =============================================================================
// 缓存
// =============================================================================

/// summary 缓存有效期（秒）：写操作 invalidate 之外的兜底，防遗漏导致脏数据。
const SUMMARY_TTL_SECS: u64 = 30;

/// 读 summary（带缓存）：未过期直接返回（0 查询）；过期/无则算一次并回填。
/// 翻页/搜索/切 tab 都走这里，summary 只在首次/写操作后算。
async fn cached_summary(
    state: &crate::state::AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
) -> FmsWorkCenterSummary {
    {
        let cache = state.fms_summary_cache.read().unwrap();
        if let Some((at, s)) = cache.as_ref() {
            if at.elapsed().as_secs() < SUMMARY_TTL_SECS {
                return s.clone();
            }
        }
    }
    // 缓存 miss：算一次（summary 内部 6 查询并行），回填
    let s = state
        .fms_work_center_service()
        .summary(ctx, db)
        .await
        .unwrap_or_default();
    *state.fms_summary_cache.write().unwrap() = Some((Instant::now(), s.clone()));
    s
}

/// 写操作 commit 后调：清缓存，下次请求重算（badge/total 及时）。
fn invalidate_fms_summary(state: &crate::state::AppState) {
    *state.fms_summary_cache.write().unwrap() = None;
}

// =============================================================================
// 显示 helper
// =============================================================================

fn fmt_amount(d: Decimal) -> String {
    format!("{:.2}", d)
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn fmt_date_opt(d: &Option<NaiveDate>) -> String {
    d.map(|x| x.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into())
}

fn currency_tag(c: &str) -> Markup {
    if c.is_empty() || c == "CNY" {
        html! {}
    } else {
        html! { span class="ml-1 text-[10px] text-muted" { (c) } }
    }
}

/// 台账行状态标：按到期日 + 未清余额判逾期/即将到期/未到期/已结清。
fn ledger_row_status(
    due_date: &Option<NaiveDate>,
    outstanding: Decimal,
    today: NaiveDate,
) -> Markup {
    if outstanding <= Decimal::ZERO {
        return status_pill("已结清", "success");
    }
    match due_date {
        Some(d) if *d < today => status_pill("逾期", "danger"),
        Some(d) if *d <= today + chrono::Duration::days(7) => status_pill("即将到期", "warn"),
        Some(_) => status_pill("未到期", "accent"),
        None => status_pill("无到期日", "muted"),
    }
}

fn status_pill(label: &str, color: &str) -> Markup {
    // color: success / danger / warn / accent / muted
    let cls = match color {
        "success" => "bg-success-bg text-success",
        "danger" => "bg-danger-bg text-danger",
        "warn" => "bg-warn-bg text-warn",
        "accent" => "bg-accent-bg text-accent",
        _ => "bg-surface text-muted",
    };
    html! {
        span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium {cls}")) {
            (label)
        }
    }
}

fn adjustment_direction_badge(d: &AdjustmentDirection) -> Markup {
    match d {
        AdjustmentDirection::Increase => status_pill("增加", "success"),
        AdjustmentDirection::Decrease => status_pill("减少", "danger"),
    }
}

fn doc_type_label(t: &abt_core::shared::enums::DocumentType) -> &'static str {
    use abt_core::shared::enums::DocumentType::*;
    match t {
        CashJournal => "CJ",
        StockShipment => "SHIP",
        StockReceipt => "RCPT",
        OutsourcingTracking => "OM",
        ArApAdjustment => "ADJ",
        _ => "DOC",
    }
}
