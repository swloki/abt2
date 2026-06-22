use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::ar_ap::model::{ArApLedgerFilter, ArApLedgerRow};
use abt_core::fms::ar_ap::ArApService;
use abt_core::fms::enums::CounterpartyType;
use abt_core::shared::types::PaginatedResult;
use rust_decimal::Decimal;

use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::ApLedgerPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query params ──

#[derive(Deserialize, Debug, Default)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub outstanding_only: Option<bool>,
}

// ── Helpers ──

fn fmt_amount(amount: Decimal) -> String {
    format!("{amount:.2}")
}

fn direction_badge(dir: &abt_core::fms::ar_ap::enums::LedgerDirection) -> (&'static str, &'static str) {
    use abt_core::fms::ar_ap::enums::LedgerDirection;
    match dir {
        LedgerDirection::Debit => ("借", "var(--danger)"),
        LedgerDirection::Credit => ("贷", "var(--success)"),
    }
}

// ── Table row component ──

fn ledger_row(item: &ArApLedgerRow) -> Markup {
    let (dir_label, dir_color) = direction_badge(&item.direction);
    let outstanding = item.amount - item.amount_applied;
    let row_class = if outstanding > Decimal::ZERO { "" } else { "text-fg-3" };

    html! {
        tr class=(row_class) {
            td class="px-4 py-3 text-sm" { (item.transaction_date) }
            td class="px-4 py-3 text-sm font-medium" { (item.party_name) }
            td class="px-4 py-3 text-sm text-fg-2" { (item.account_code) " " (item.account_name) }
            td class="px-4 py-3 text-sm text-fg-2" { (item.source_doc_no) }
            td class="px-4 py-3 text-sm font-mono" style=(format!("color:{dir_color}")) { (dir_label) }
            td class="px-4 py-3 text-sm font-mono text-right" { "¥" (fmt_amount(item.amount)) }
            td class="px-4 py-3 text-sm font-mono text-right" { "¥" (fmt_amount(item.amount_applied)) }
            td class="px-4 py-3 text-sm font-mono text-right font-semibold" {
                @if outstanding > Decimal::ZERO {
                    span style=(format!("color:{}", dir_color)) { "¥" (fmt_amount(outstanding)) }
                } @else {
                    span class="text-fg-3" { "¥0.00" }
                }
            }
            td class="px-4 py-3 text-sm text-fg-2" {
                @if let Some(due) = item.due_date { (due) } @else { "—" }
            }
            td class="px-4 py-3 text-sm text-fg-2" { (item.period) }
        }
    }
}

// ── Table component ──

fn ledger_table(items: &[ArApLedgerRow], total: u64, page: u32, page_size: u32, query_string: &str) -> Markup {
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
    html! {
        div id="data-card" class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "日期" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "供应商" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "科目" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "单据" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "方向" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "金额" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "已核销" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "未清余额" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "到期日" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "期间" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for item in items {
                            (ledger_row(item))
                        }
                    }
                }
            }
            @if total > page_size as u64 {
                (pagination(ApLedgerPath::PATH, query_string, total, page, total_pages))
            }
        }
    }
}

// ── Handlers ──

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

    let filter = ArApLedgerFilter {
        party_type: Some(CounterpartyType::Supplier),
        outstanding_only: q.outstanding_only.unwrap_or(true),
        ..Default::default()
    };

    let page_params = abt_core::shared::types::PageParams::new(page, page_size);
    let result = svc.list_ledger(&service_ctx, &mut conn, filter, page_params).await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, page, page_size));

    let query_string = if q.outstanding_only.unwrap_or(true) { "outstanding_only=true" } else { "" };

    let content = html! {
        div {
            div class="flex items-center justify-between mb-5" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "应付台账" }
                div class="flex gap-3" {
                    a href=(ApLedgerPath) class=(format!("btn btn-sm {}", if q.outstanding_only.unwrap_or(true) { "btn-primary" } else { "btn-outline" }))
                        hx-get=(ApLedgerPath) hx-target="#data-card" hx-select="#data-card" hx-push-url="true"
                        hx-vals=r#"{"outstanding_only":true}"# { "只看未清" }
                    a href=(ApLedgerPath) class=(format!("btn btn-sm {}", if !q.outstanding_only.unwrap_or(true) { "btn-primary" } else { "btn-outline" }))
                        hx-get=(ApLedgerPath) hx-target="#data-card" hx-select="#data-card" hx-push-url="true"
                        hx-vals=r#"{"outstanding_only":false}"# { "全部" }
                }
            }
            (ledger_table(&result.items, result.total, page, page_size, query_string))
        }
    };

    let page_html = admin_page(
        is_htmx,
        "应付台账",
        &claims,
        "finance",
        ApLedgerPath::PATH,
        "财务管理",
        None,
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}
