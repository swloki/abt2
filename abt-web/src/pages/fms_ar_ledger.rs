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

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::ArLedgerPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query params ──

#[derive(Deserialize, Debug, Default)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub keyword: Option<String>,
    pub outstanding_only: Option<bool>,
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
            div class=(format!("w-11 h-11 rounded-md grid place-items-center shrink-0 {}", icon_cls)) {
                (icon_svg)
            }
            div class="flex-1 min-w-0" {
                div class="text-sm text-muted" { (title) }
                div class=(format!("text-2xl font-bold font-mono tabular-nums mt-1 {}", value_cls)) {
                    (value)
                }
            }
        }
    }
}

fn summary_cards(s: &LedgerSummary) -> Markup {
    html! {
        div class="grid grid-cols-4 gap-4 mb-6" {
            (stat_card("应收总额", &format!("¥{}", fmt_amount(s.total_amount)), "text-fg",
                icon::dollar_icon("w-5 h-5 text-accent"), "bg-accent-bg"))
            (stat_card("未清余额", &format!("¥{}", fmt_amount(s.total_outstanding)), "text-success",
                icon::check_circle_icon("w-5 h-5 text-success"), "bg-success/10"))
            (stat_card("逾期金额", &format!("¥{}", fmt_amount(s.total_overdue)), "text-danger",
                icon::alert_triangle_icon("w-5 h-5 text-danger"), "bg-danger/10"))
            (stat_card("7天内到期", &format!("¥{}", fmt_amount(s.due_within_7d)), "text-warn",
                icon::clock_icon("w-5 h-5 text-warn"), "bg-warn/10"))
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
            td class="px-4 py-3 text-sm whitespace-nowrap" { (item.transaction_date.format("%Y-%m-%d")) }
            td class="px-4 py-3 text-sm font-medium" { (item.party_name) }
            td class="px-4 py-3 text-sm text-accent font-mono" { (item.source_doc_no) }
            td class="px-4 py-3 text-sm text-fg-2 whitespace-nowrap" {
                @if let Some(due) = item.due_date { (due.format("%Y-%m-%d")) } @else { "—" }
            }
            td class="px-4 py-3 text-sm font-mono tabular-nums text-right" { "¥" (fmt_amount(item.amount)) }
            td class="px-4 py-3 text-sm font-mono tabular-nums text-right" { "¥" (fmt_amount(item.amount_applied)) }
            td class=(format!("px-4 py-3 text-sm font-mono tabular-nums text-right font-semibold {}", outstanding_cls)) {
                "¥" (fmt_amount(outstanding))
            }
            td class="px-4 py-3 text-sm" {
                @if let Some((label, txt, bg)) = status {
                    span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {} {}", txt, bg)) { (label) }
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
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "日期" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "客户" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "单据号" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "到期日" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "应收金额" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "已核销" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "未清余额" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "状态" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for item in items {
                            (ledger_row(item, today))
                        }
                        @if items.is_empty() {
                            tr { td colspan="8" class="px-4 py-12 text-center text-muted text-sm" { "暂无应收记录" } }
                        }
                    }
                }
            }
            @if total > page_size as u64 {
                (pagination(ArLedgerPath::PATH, query_string, total, page, total_pages))
            }
        }
    }
}

/// 筛选区 + 表格（HTMX 局部刷新 #data-card）
fn filter_and_table(
    result: &PaginatedResult<ArApLedgerRow>,
    keyword_val: &str,
    outstanding_only: bool,
    today: chrono::NaiveDate,
    query_string: &str,
) -> Markup {
    let active_cls = "inline-flex items-center gap-1.5 px-4 py-1.5 text-sm font-semibold cursor-pointer bg-bg text-accent rounded-sm";
    let inactive_cls = "inline-flex items-center gap-1.5 px-4 py-1.5 text-sm cursor-pointer bg-transparent border-none text-muted rounded-sm hover:text-fg transition-colors";

    html! {
        // 筛选 form（HTMX：搜索框 keyup 触发）
        form class="flex items-center gap-3 mb-5 flex-wrap"
            hx-get=(ArLedgerPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#data-card"
            hx-select="#data-card"
            hx-swap="outerHTML"
            hx-push-url="true" {
            input type="hidden" name="outstanding_only" value=(outstanding_only);
            // 搜索框
            div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted" {
                (icon::search_icon(""))
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                    type="text" name="keyword" placeholder="搜索客户名称…"
                    value=(keyword_val);
            }
            // 只看未清 / 全部 toggle（hx-include keyword input 保持搜索词）
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                a class=(if outstanding_only { active_cls } else { inactive_cls })
                    hx-get=(ArLedgerPath::PATH) hx-vals=r#"{"outstanding_only":"true"}"#
                    hx-target="#data-card" hx-select="#data-card" hx-swap="outerHTML" hx-push-url="true"
                    hx-include="input[name=keyword]" { "只看未清" }
                a class=(if !outstanding_only { active_cls } else { inactive_cls })
                    hx-get=(ArLedgerPath::PATH) hx-vals=r#"{"outstanding_only":"false"}"#
                    hx-target="#data-card" hx-select="#data-card" hx-swap="outerHTML" hx-push-url="true"
                    hx-include="input[name=keyword]" { "全部" }
            }
        }
        (ledger_table(&result.items, today, result.total, result.page, result.page_size, query_string))
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
    let keyword_val = q.keyword.as_deref().unwrap_or("").to_string();

    let filter = ArApLedgerFilter {
        party_type: Some(CounterpartyType::Customer),
        outstanding_only,
        keyword: if keyword_val.is_empty() { None } else { Some(keyword_val.clone()) },
        ..Default::default()
    };

    let summary = svc.ledger_summary(&service_ctx, &mut conn, filter.clone()).await.unwrap_or_default();
    let result = svc.list_ledger(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page, page_size)).await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, page, page_size));

    // 分页 query_string（保持 keyword + outstanding_only）
    let mut parts: Vec<String> = Vec::new();
    if !keyword_val.is_empty() { parts.push(format!("keyword={}", url_encode(&keyword_val))); }
    if outstanding_only { parts.push("outstanding_only=true".into()); }
    let query_string = if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) };

    let content = html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "应收台账" }
            }
            (summary_cards(&summary))
            (filter_and_table(&result, &keyword_val, outstanding_only, today, &query_string))
        }
    };

    let page_html = admin_page(is_htmx, "应收台账", &claims, "finance", ArLedgerPath::PATH, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
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
