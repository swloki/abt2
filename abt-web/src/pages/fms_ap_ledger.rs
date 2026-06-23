use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::fms::ar_ap::model::{ArApLedgerFilter, ArApLedgerRow, LedgerSummary};
use abt_core::fms::ar_ap::ArApService;
use abt_core::fms::enums::CounterpartyType;
use abt_core::shared::types::PaginatedResult;

use crate::components::export_button;
use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::ApLedgerPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query params ──

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub keyword: Option<String>,
    pub outstanding_only: Option<bool>,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
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

// ── Stat card ──

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
                    "应付总额",
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

fn ledger_row(item: &ArApLedgerRow, today: chrono::NaiveDate) -> Markup {
    let outstanding = item.amount - item.amount_applied;
    let status = row_status(item, today);
    let row_cls = if outstanding <= Decimal::ZERO { "text-fg-3" } else { "" };
    let outstanding_cls = if outstanding > Decimal::ZERO { "text-fg" } else { "text-fg-3" };

    html! {
        tr class=(row_cls) {
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

fn ledger_table(items: &[ArApLedgerRow], today: chrono::NaiveDate, total: u64, page: u32, page_size: u32, query_string: &str) -> Markup {
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
    html! {
        div id="data-card" class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table w-full" {
                    thead {
                        tr class="border-b border-border-soft" {
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "日期" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "供应商" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "单据号" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "采购单号" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "产品" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "到期日" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "应付金额" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "已核销" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "未清余额" }
                            th  class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider"
                            { "状态" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for item in items { (ledger_row(item, today)) }
                        @if items.is_empty() {
                            tr {
                                td colspan="10" class="px-4 py-12 text-center text-muted text-sm" {
                                    "暂无应付记录"
                                }
                            }
                        }
                    }
                }
            }
            @if total > page_size as u64 {
                ({
                    pagination(
                        ApLedgerPath::PATH,
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

fn filter_input(input_type: &str, name: &str, placeholder: &str, value: &str) -> Markup {
    html! {
        input
            type=(input_type)
            name=(name)
            class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
            placeholder=(placeholder)
            value=(value);
    }
}

fn filter_and_table(
    result: &PaginatedResult<ArApLedgerRow>,
    q: &ListQuery,
    outstanding_only: bool,
    today: chrono::NaiveDate,
    query_string: &str,
) -> Markup {
    let active_cls = "inline-flex items-center gap-1.5 px-4 py-1.5 text-sm font-semibold cursor-pointer bg-bg text-accent rounded-sm";
    let inactive_cls = "inline-flex items-center gap-1.5 px-4 py-1.5 text-sm cursor-pointer bg-transparent border-none text-muted rounded-sm hover:text-fg transition-colors";
    let keyword = q.keyword.as_deref().unwrap_or("");
    let start = q.start_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
    let end = q.end_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();

    html! {
        form id="ap-filter-form"
            class="flex items-center gap-3 mb-5 flex-wrap"
            hx-get=(ApLedgerPath::PATH)
            hx-trigger="change, keyup changed delay:300ms"
            hx-target="#data-card"
            hx-select="#data-card"
            hx-swap="outerHTML"
            hx-push-url="true"
        {
            input type="hidden" name="outstanding_only" value=(outstanding_only);
            div class="relative flex-1 min-w-[200px] max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
            {
                (icon::search_icon(""))
                input
                    class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                    type="text"
                    name="keyword"
                    placeholder="供应商名称"
                    value=(keyword);
            }
            (filter_input("date", "start_date", "开始日期", &start))
            span class="text-muted text-sm self-center" { "—" }
            (filter_input("date", "end_date", "结束日期", &end))
            (filter_input("text", "doc_no", "发生单号", q.doc_no.as_deref().unwrap_or("")))
            (filter_input("text", "product_code", "产品编码", q.product_code.as_deref().unwrap_or("")))
            (filter_input("text", "product_name", "产品名称", q.product_name.as_deref().unwrap_or("")))
            (filter_input("text", "rep_name", "采购员", q.rep_name.as_deref().unwrap_or("")))
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5"
            {
                a   class=(if outstanding_only { active_cls } else { inactive_cls })
                    hx-get=(ApLedgerPath::PATH)
                    hx-vals=r#"{"outstanding_only":"true"}"#
                    hx-target="#data-card"
                    hx-select="#data-card"
                    hx-swap="outerHTML"
                    hx-push-url="true"
                    hx-include="#ap-filter-form input:not([type=hidden])"
                { "只看未清" }
                a   class=(if !outstanding_only { active_cls } else { inactive_cls })
                    hx-get=(ApLedgerPath::PATH)
                    hx-vals=r#"{"outstanding_only":"false"}"#
                    hx-target="#data-card"
                    hx-select="#data-card"
                    hx-swap="outerHTML"
                    hx-push-url="true"
                    hx-include="#ap-filter-form input:not([type=hidden])"
                { "全部" }
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
            )
        })
    }
}

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_list(
    _path: ApLedgerPath,
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
        party_type: Some(CounterpartyType::Supplier),
        outstanding_only,
        keyword: opt_string(&q.keyword),
        doc_no: opt_string(&q.doc_no),
        product_code: opt_string(&q.product_code),
        product_name: opt_string(&q.product_name),
        rep_name: opt_string(&q.rep_name),
        start_date: q.start_date,
        end_date: q.end_date,
        ..Default::default()
    };

    let summary = svc.ledger_summary(&service_ctx, &mut conn, filter.clone()).await.unwrap_or_default();
    let result = svc.list_ledger(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page, page_size)).await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, page, page_size));

    let mut parts: Vec<String> = Vec::new();
    push_param(&mut parts, "keyword", &q.keyword);
    if outstanding_only { parts.push("outstanding_only=true".into()); }
    if let Some(d) = q.start_date { parts.push(format!("start_date={d}")); }
    if let Some(d) = q.end_date { parts.push(format!("end_date={d}")); }
    push_param(&mut parts, "doc_no", &q.doc_no);
    push_param(&mut parts, "product_code", &q.product_code);
    push_param(&mut parts, "product_name", &q.product_name);
    push_param(&mut parts, "rep_name", &q.rep_name);
    let query_string = if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) };

    let content = html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "应付台账" }
                (export_button::export_button("导出明细表", "ap-ledger-detail", Some("#ap-filter-form")))
            }
            (summary_cards(&summary))
            ({
                filter_and_table(
                    &result,
                    &q,
                    outstanding_only,
                    today,
                    &query_string,
                )
            })
        }
    };

    let page_html = admin_page(is_htmx, "应付台账", &claims, "finance", ApLedgerPath::PATH, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}

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
