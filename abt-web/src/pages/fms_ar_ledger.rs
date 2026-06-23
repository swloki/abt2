use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::fms::ar_ap::model::{ArApLedgerFilter, ArApLedgerRow, LedgerDetailItem, LedgerSummary};
use abt_core::fms::ar_ap::ArApService;
use abt_core::shared::identity::UserService;
use abt_core::fms::enums::CounterpartyType;
use abt_core::shared::types::PaginatedResult;

use crate::components::drawer::drawer_with_footer;
use crate::components::export_button;
use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{ArLedgerDetailPath, ArLedgerPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query params ──

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub keyword: Option<String>,
    pub outstanding_only: Option<bool>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub doc_no: Option<String>,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub rep_name: Option<String>,
}

// ── Helpers ──

fn fmt_amount(amount: Decimal) -> String {
    format!("{amount:.2}")
}

/// 行状态（逾期基准 = due_date）：(label, text-cls, bg-cls)
fn row_status(item: &ArApLedgerRow, today: chrono::NaiveDate) -> Option<(&'static str, &'static str, &'static str)> {
    let outstanding = item.amount - item.amount_applied;
    if outstanding <= Decimal::ZERO {
        return None;
    }
    match item.due_date {
        Some(due) if due < today => Some(("逾期", "text-danger", "bg-danger/10")),
        Some(due) if due <= today + chrono::Duration::days(7) => Some(("即将到期", "text-warn", "bg-warn/10")),
        _ => None,
    }
}

// ── Stat card（参考 fms_dashboard）──

fn stat_card(title: &str, value: &str, value_cls: &str, icon_svg: Markup, icon_cls: &str) -> Markup {
    html! {
        div class="data-card flex items-center gap-4 p-5" {
            div class=({
                    format!(
                        "w-11 h-11 rounded-md grid place-items-center shrink-0 {}",
                        icon_cls,
                    )
                })
            { (icon_svg) }
            div class="flex-1 min-w-0" {
                div class="text-sm text-muted" { (title) }
                div class=({
                    format!(
                        "text-2xl font-bold font-mono tabular-nums mt-1 {}",
                        value_cls,
                    )
                }) { (value) }
            }
        }
    }
}

fn summary_cards(s: &LedgerSummary) -> Markup {
    html! {
        div class="grid grid-cols-4 gap-4 mb-6" {
            ({
                stat_card(
                    "应收总额",
                    &format!("¥{}", fmt_amount(s.total_amount)),
                    "text-fg",
                    icon::dollar_icon("w-5 h-5 text-accent"),
                    "bg-accent-bg",
                )
            })
            ({
                stat_card(
                    "未清余额",
                    &format!("¥{}", fmt_amount(s.total_outstanding)),
                    "text-success",
                    icon::check_circle_icon("w-5 h-5 text-success"),
                    "bg-success/10",
                )
            })
            ({
                stat_card(
                    "逾期金额",
                    &format!("¥{}", fmt_amount(s.total_overdue)),
                    "text-danger",
                    icon::alert_triangle_icon("w-5 h-5 text-danger"),
                    "bg-danger/10",
                )
            })
            ({
                stat_card(
                    "7天内到期",
                    &format!("¥{}", fmt_amount(s.due_within_7d)),
                    "text-warn",
                    icon::clock_icon("w-5 h-5 text-warn"),
                    "bg-warn/10",
                )
            })
        }
    }
}

// ── Table ──

fn ledger_row(item: &ArApLedgerRow, today: chrono::NaiveDate, detail_path: &str) -> Markup {
    let outstanding = item.amount - item.amount_applied;
    let status = row_status(item, today);
    let row_cls = if outstanding <= Decimal::ZERO { "text-fg-3 cursor-pointer" } else { "cursor-pointer" };
    let outstanding_cls = if outstanding > Decimal::ZERO { "text-fg" } else { "text-fg-3" };
    let detail_url = format!("{detail_path}?id={}", item.id);

    html! {
        tr class=(row_cls)
            hx-get=(detail_url)
            hx-target="#ar-drawer-content"
            hx-swap="innerHTML"
            _="on 'htmx:afterRequest' add .open to #ar-drawer"
        {
            td class="px-4 py-3 text-sm whitespace-nowrap" {
                (item.transaction_date.format("%Y-%m-%d"))
            }
            td class="px-4 py-3 text-sm font-medium" { (item.party_name) }
            td class="px-4 py-3 text-sm text-accent font-mono" { (item.source_doc_no) }
            td class="px-4 py-3 text-sm text-fg-2 font-mono" {
                @if let Some(no) = item.upstream_doc_no.as_deref() { (no) } @else { "—" }
            }
            td class="px-4 py-3 text-sm text-fg-2 max-w-[180px] truncate" title=(item.product_summary.as_deref().unwrap_or("")) {
                @if let Some(p) = item.product_summary.as_deref() { (p) } @else { "—" }
            }
            td class="px-4 py-3 text-sm text-fg-2 whitespace-nowrap" {
                @if let Some(due) = item.due_date { (due.format("%Y-%m-%d")) } @else { "—" }
            }
            td class="px-4 py-3 text-sm font-mono tabular-nums text-right" {
                "¥"
                (fmt_amount(item.amount))
            }
            td class="px-4 py-3 text-sm font-mono tabular-nums text-right" {
                "¥"
                (fmt_amount(item.amount_applied))
            }
            td  class=({
                    format!(
                        "px-4 py-3 text-sm font-mono tabular-nums text-right font-semibold {}",
                        outstanding_cls,
                    )
                })
            { "¥" (fmt_amount(outstanding)) }
            td class="px-4 py-3 text-sm" {
                @if let Some((label, txt, bg)) = status {
                    span
                        class=({
                            format!(
                                "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {} {}",
                                txt,
                                bg,
                            )
                        })
                    { (label) }
                } @else if outstanding <= Decimal::ZERO {
                    span class="text-fg-3 text-xs" { "已结清" }
                } @else {
                    span class="text-fg-3 text-xs" { "—" }
                }
            }
        }
    }
}

fn ledger_table(items: &[ArApLedgerRow], today: chrono::NaiveDate, total: u64, page: u32, page_size: u32, query_string: &str, detail_path: &str) -> Markup {
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
    html! {
        div class="data-card mt-4" {
            div class="overflow-x-auto" {
                table class="data-table w-full" {
                    thead {
                        tr class="border-b border-border-soft" {
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "日期" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "客户" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "单据号" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "销售单号" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "产品" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "到期日" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "应收金额" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "已核销" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "未清余额" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "状态" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for item in items { (ledger_row(item, today, detail_path)) }
                        @if items.is_empty() {
                            tr {
                                td colspan="10" class="px-4 py-12 text-center text-muted text-sm" {
                                    "暂无应收记录"
                                }
                            }
                        }
                    }
                }
            }
            @if total > page_size as u64 {
                ({
                    pagination(
                        ArLedgerPath::PATH,
                        query_string,
                        total,
                        page,
                        total_pages,
                    )
                })
            }
        }
    }
}

/// 筛选区 + 表格（HTMX 局部刷新 #data-card）
fn filter_and_table(
    result: &PaginatedResult<ArApLedgerRow>,
    q: &ListQuery,
    outstanding_only: bool,
    today: chrono::NaiveDate,
    query_string: &str,
    detail_path: &str,
    buyers: &[String],
) -> Markup {
    let active_cls = "inline-flex items-center px-3 py-1 text-sm font-semibold cursor-pointer bg-bg text-accent rounded-sm";
    let inactive_cls = "inline-flex items-center px-3 py-1 text-sm cursor-pointer bg-transparent border-none text-muted rounded-sm hover:text-fg transition-colors";
    let keyword = q.keyword.as_deref().unwrap_or("");
    let start = q.start_date.clone().unwrap_or_default();
    let end = q.end_date.clone().unwrap_or_default();
    let ti = "px-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-colors duration-150 focus:border-accent";

    let has_filter = q.start_date.is_some() || q.end_date.is_some()
        || q.doc_no.as_deref().is_some_and(|s| !s.is_empty())
        || q.rep_name.as_deref().is_some_and(|s| !s.is_empty());
    let panel_cls = if has_filter { "filter-advanced-inner " } else { "filter-advanced-inner hidden" };
    let arrow_cls = if has_filter { "filter-arrow inline-block transition-transform rotate-180" }
                    else { "filter-arrow inline-block transition-transform" };

    html! {
        div id="data-card" {
        form id="ar-filter-form"
            class="mb-4"
            hx-get=(ArLedgerPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#data-card"
            hx-select="#data-card"
            hx-swap="outerHTML"
            hx-push-url="true"
        {
            input type="hidden" name="outstanding_only" value=(outstanding_only);
            // ── filter-card 容器 ──
            div class="border border-border-soft rounded-md bg-white mb-4"
            {
                // 主筛选行
                div class="flex items-center gap-2.5 px-3 py-2.5 flex-wrap"
                {
                    // 产品名称
                    div class="relative w-48 icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-3.5 icon:h-3.5 icon:text-muted" {
                        (icon::search_icon(""))
                        input class="w-full pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-colors duration-150 focus:border-accent search-input"
                            type="text" name="product_name" id="product_name" hx-preserve
                            placeholder="产品名称" value=(q.product_name.as_deref().unwrap_or(""));
                    }
                    // 客户（搜索型 select）
                    (crate::components::customer_search::customer_search_field(
                        "ar-keyword", "ar-keyword-display", "ar-customer-panel", "ar-customer-results", "keyword", keyword, "客户"
                    ))
                    // 产品编码
                    input type="text" id="product_code" name="product_code" hx-preserve
                        class=(format!("{} w-32 search-input", ti)) placeholder="产品编码" value=(q.product_code.as_deref().unwrap_or(""));
                    // toggle
                    div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5"
                    {
                        a class=(if outstanding_only { active_cls } else { inactive_cls })
                            hx-get=(ArLedgerPath::PATH) hx-vals=r#"{"outstanding_only":"true"}"#
                            hx-target="#data-card" hx-select="#data-card" hx-swap="outerHTML" hx-push-url="true"
                            hx-include="#ar-filter-form input:not([type=hidden])"
                        { "只看未清" }
                        a class=(if !outstanding_only { active_cls } else { inactive_cls })
                            hx-get=(ArLedgerPath::PATH) hx-vals=r#"{"outstanding_only":"false"}"#
                            hx-target="#data-card" hx-select="#data-card" hx-swap="outerHTML" hx-push-url="true"
                            hx-include="#ar-filter-form input:not([type=hidden])"
                        { "全部" }
                    }
                    // 高级筛选 + 重置（右对齐）
                    div class="flex items-center gap-2 ml-auto" {
                        button type="button"
                            class="inline-flex items-center gap-1 text-xs text-fg-2 hover:text-accent cursor-pointer select-none border-none bg-transparent p-0 transition-colors"
                            _="on click toggle .hidden on #ar-filter-panel then toggle .rotate-180 on .filter-arrow"
                        {
                            "高级筛选 "
                            span class=(arrow_cls) { "▾" }
                        }
                        a class="inline-flex items-center gap-1 text-xs text-fg-2 hover:text-accent cursor-pointer select-none no-underline transition-colors"
                            hx-get=(ArLedgerPath::PATH)
                            { (icon::refresh_icon("w-3.5 h-3.5")) " 重置" }
                    }
                }
                // 高级筛选面板（折叠）
                div id="ar-filter-panel" class=(panel_cls)
                {
                    div class="flex items-center gap-3 px-4 pb-3 flex-wrap border-t border-border-soft pt-3"
                    {
                        span class="text-xs text-fg-2" { "发生日期" }
                        input type="date" id="start_date" name="start_date" hx-preserve class=(ti) value=(start);
                        span class="text-fg-3 text-xs" { "至" }
                        input type="date" id="end_date" name="end_date" hx-preserve class=(ti) value=(end);
                        span class="text-fg-3 mx-1" { "|" }
                        span class="text-xs text-fg-2" { "发生单号" }
                        input type="text" id="doc_no" name="doc_no" hx-preserve class=(ti) placeholder="模糊搜索" value=(q.doc_no.as_deref().unwrap_or(""));
                        span class="text-fg-3 mx-1" { "|" }
                        span class="text-xs text-fg-2" { "销售经理" }
                        select id="rep_name" name="rep_name" hx-preserve class=(ti) {
                            option value="" { "全部" }
                            @for name in buyers {
                                option value=(name) selected[(q.rep_name.as_deref() == Some(name.as_str()))] { (name) }
                            }
                        }
                    }
                }
            }
        }
        ({
            ledger_table(
                &result.items,
                today,
                result.total,
                result.page,
                result.page_size,
                query_string,
                detail_path,
            )
        })
        }
    }
}

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_list(
    _path: ArLedgerPath,
    ctx: RequestContext,
    Query(q): Query<ListQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.ar_ap_service();

    let page = q.page.unwrap_or(1).max(1);
    let page_size = 20u32;
    let today = chrono::Utc::now().date_naive();
    let outstanding_only = q.outstanding_only.unwrap_or(true);

    let filter = ArApLedgerFilter {
        party_type: Some(CounterpartyType::Customer),
        outstanding_only,
        keyword: opt_string(&q.keyword),
        doc_no: opt_string(&q.doc_no),
        product_code: opt_string(&q.product_code),
        product_name: opt_string(&q.product_name),
        rep_name: opt_string(&q.rep_name),
        start_date: q.start_date.as_deref().and_then(|s| s.trim().parse().ok()),
        end_date: q.end_date.as_deref().and_then(|s| s.trim().parse().ok()),
        ..Default::default()
    };

    let summary = svc.ledger_summary(&service_ctx, &mut conn, filter.clone()).await.unwrap_or_default();
    let result = svc.list_ledger(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page, page_size)).await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, page, page_size));

    let buyers: Vec<String> = state.user_service()
        .list_users_by_departments(&service_ctx, &mut conn, &["XIAOSHOU"]).await
        .unwrap_or_default().into_iter()
        .filter(|u: &abt_core::shared::identity::model::UserWithRoles| u.user.is_active)
        .filter_map(|u: abt_core::shared::identity::model::UserWithRoles| u.user.display_name)
        .filter(|n: &String| !n.is_empty())
        .collect();

    let mut parts: Vec<String> = Vec::new();
    push_param(&mut parts, "keyword", &q.keyword);
    if outstanding_only { parts.push("outstanding_only=true".into()); }
    push_param(&mut parts, "start_date", &q.start_date);
    push_param(&mut parts, "end_date", &q.end_date);
    push_param(&mut parts, "doc_no", &q.doc_no);
    push_param(&mut parts, "product_code", &q.product_code);
    push_param(&mut parts, "product_name", &q.product_name);
    push_param(&mut parts, "rep_name", &q.rep_name);
    let query_string = if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) };

    let content = html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "应收台账" }
                (export_button::export_button("导出明细表", "ar-ledger-detail", Some("#ar-filter-form")))
            }
            (summary_cards(&summary))
            ({
                filter_and_table(
                    &result,
                    &q,
                    outstanding_only,
                    today,
                    &query_string,
                    ArLedgerDetailPath::PATH,
                    &buyers,
                )
            })
            ({
                drawer_with_footer(
                    "ar-drawer",
                    "应收台账详情",
                    html! { div id="ar-drawer-content" {} },
                    html! {
                        button
                            type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "关闭" }
                    },
                )
            })
        }
    };

    let page_html = admin_page(is_htmx, "应收台账", &claims, "finance", ArLedgerPath::PATH, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}

// ── Detail handler（drawer）──

#[derive(Deserialize, Debug)]
pub(crate) struct DetailQuery {
    id: i64,
}

fn detail_field(label: &str, value: &str) -> Markup {
    html! {
        div class="flex flex-col gap-0.5" {
            span class="text-xs text-fg-2" { (label) }
            span class="text-sm text-fg font-medium" { (value) }
        }
    }
}

#[require_permission("FMS", "read")]
pub async fn get_detail(
    _path: ArLedgerDetailPath,
    ctx: RequestContext,
    Query(q): Query<DetailQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.ar_ap_service();

    let detail = svc
        .get_ledger_detail(&service_ctx, &mut conn, q.id)
        .await?;

    let content = match detail {
        Some((row, items)) => {
            let upstream = row.upstream_doc_no.clone().unwrap_or_else(|| "—".into());
            let due_str = row.due_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
            let outstanding = row.amount - row.amount_applied;
            let product_summ = row.product_summary.clone().unwrap_or_else(|| "—".into());
            let transaction_str = row.transaction_date.format("%Y-%m-%d").to_string();
            let amount_str = format!("¥{:.2}", row.amount);
            let applied_str = format!("¥{:.2}", row.amount_applied);
            let outstanding_str = format!("¥{:.2}", outstanding);
            html! {
                div {
                    h3 class="text-sm font-semibold text-fg mb-3" { "基本信息" }
                    div class="grid grid-cols-2 gap-3 mb-5" {
                        (detail_field("往来方", &row.party_name))
                        (detail_field("发生单号", &row.source_doc_no))
                        (detail_field("销售单号", &upstream))
                        (detail_field("发生日期", &transaction_str))
                        (detail_field("到期日", &due_str))
                        (detail_field("应收金额", &amount_str))
                        (detail_field("已核销", &applied_str))
                        (detail_field("未清余额", &outstanding_str))
                        (detail_field("产品", &product_summ))
                    }
                    h3 class="text-sm font-semibold text-fg mb-3" { "产品明细" }
                    @if items.is_empty() {
                        p class="text-muted text-sm" { "无产品明细" }
                    } @else {
                        div class="overflow-x-auto" {
                            table class="data-table w-full text-sm" {
                                thead {
                                    tr class="border-b border-border-soft" {
                                        th class="px-3 py-2 text-left text-xs font-medium text-fg-2 uppercase" { "产品编码" }
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
        },
        None => html! {
            div class="text-center text-muted py-8" { "未找到该台账记录" }
        },
    };

    Ok(Html(content.into_string()))
}


/// 简单 URL 编码（分页 query_string 用，保留 keyword 中文）
fn url_encode(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            write!(out, "{}", b as char).unwrap();
        } else {
            write!(out, "%{:02X}", b).unwrap();
        }
    }
    out
}

/// Option<String> → 空字符串归一为 None
fn opt_string(s: &Option<String>) -> Option<String> {
    s.as_deref().filter(|s| !s.is_empty()).map(str::to_string)
}

/// 把非空字符串参数加入 query parts（URL 编码），供分页保持筛选
fn push_param(parts: &mut Vec<String>, key: &str, val: &Option<String>) {
    if let Some(v) = val {
        let v = v.trim();
        if !v.is_empty() {
            parts.push(format!("{}={}", key, url_encode(v)));
        }
    }
}
