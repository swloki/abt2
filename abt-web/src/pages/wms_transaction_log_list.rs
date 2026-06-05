use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::inventory::model::TransactionLogFilter;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::enums::TransactionType;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::wms_transaction_log::{TransactionListPath, TransactionTablePath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TransactionLogQueryParams {
    pub keyword: Option<String>,
    pub transaction_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_transaction_list(
    _path: TransactionListPath,
    ctx: RequestContext,
    Query(params): Query<TransactionLogQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inventory_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.query_logs(&service_ctx, &mut conn, filter, page_num, 20).await?;

    let content = transaction_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "库存事务日志",
        &claims,
        "inventory",
        TransactionListPath::PATH,
        "库存管理",
        Some("库存事务日志"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "read")]
pub async fn get_transaction_table(
    _path: TransactionTablePath,
    ctx: RequestContext,
    Query(params): Query<TransactionLogQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.query_logs(&service_ctx, &mut conn, filter, page_num, 20).await?;

    Ok(Html(transaction_table_fragment(&result, &params).into_string()))
}

// ── Helpers ──

fn build_filter(params: &TransactionLogQueryParams) -> TransactionLogFilter {
    TransactionLogFilter {
        product_id: None,
        product_name: None,
        product_code: params.keyword.clone(),
        bin_id: None,
        warehouse_id: params.warehouse_id,
        transaction_type: if params.transaction_type.as_deref() == Some("") || params.transaction_type.is_none() {
            None
        } else {
            params.transaction_type.clone()
        },
        start_date: None,
        end_date: None,
    }
}

fn txn_type_label(t: &TransactionType) -> &'static str {
    match t {
        TransactionType::PurchaseReceipt => "采购入库",
        TransactionType::ProductionReceipt => "生产入库",
        TransactionType::SalesShipment => "销售出库",
        TransactionType::MaterialIssue => "生产领料",
        TransactionType::MaterialReturn => "生产退料",
        TransactionType::Backflush => "系统倒冲",
        TransactionType::Transfer => "调拨",
        TransactionType::FormConversion => "形态转换",
        TransactionType::Adjustment => "盘点调整",
        TransactionType::Lock => "锁库",
        TransactionType::Unlock => "解锁",
        TransactionType::Scrap => "报废",
    }
}

fn txn_type_class(t: &TransactionType) -> &'static str {
    match t {
        TransactionType::PurchaseReceipt
        | TransactionType::ProductionReceipt
        | TransactionType::MaterialReturn
        | TransactionType::Unlock => "txn-type-in",
        TransactionType::SalesShipment
        | TransactionType::MaterialIssue
        | TransactionType::Backflush
        | TransactionType::Scrap => "txn-type-out",
        TransactionType::Transfer => "txn-type-move",
        TransactionType::Adjustment => "txn-type-adjust",
        TransactionType::Lock => "txn-type-lock",
        TransactionType::FormConversion => "txn-type-convert",
    }
}

// ── Components ──

fn transaction_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory::model::TransactionDetailView>,
    params: &TransactionLogQueryParams,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "库存事务日志" }
                div class="page-actions" {
                    span style="font-size:var(--text-xs);color:var(--muted);display:flex;align-items:center;gap:var(--space-2);background:var(--surface);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-1) var(--space-3)" {
                        (crate::components::icon::lock_icon("w-4 h-4"))
                        "Append-Only：事务日志仅追加，不可修改历史记录"
                    }
                }
            }

            (transaction_table_fragment(result, params))
        }
    }
}

fn transaction_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory::model::TransactionDetailView>,
    params: &TransactionLogQueryParams,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="txn-log-panel" {
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索单号/产品…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(TransactionTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .txn-log-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="transaction_type"
                    hx-get=(TransactionTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .txn-log-panel"
                    hx-swap="outerHTML" {
                    option value="" selected[params.transaction_type.is_none() || params.transaction_type.as_deref() == Some("")] { "全部类型" }
                    option value="1" selected[params.transaction_type.as_deref() == Some("1")] { "采购入库" }
                    option value="2" selected[params.transaction_type.as_deref() == Some("2")] { "生产入库" }
                    option value="3" selected[params.transaction_type.as_deref() == Some("3")] { "销售出库" }
                    option value="4" selected[params.transaction_type.as_deref() == Some("4")] { "生产领料" }
                    option value="5" selected[params.transaction_type.as_deref() == Some("5")] { "生产退料" }
                    option value="6" selected[params.transaction_type.as_deref() == Some("6")] { "系统倒冲" }
                    option value="7" selected[params.transaction_type.as_deref() == Some("7")] { "调拨" }
                    option value="8" selected[params.transaction_type.as_deref() == Some("8")] { "形态转换" }
                    option value="9" selected[params.transaction_type.as_deref() == Some("9")] { "盘点调整" }
                    option value="10" selected[params.transaction_type.as_deref() == Some("10")] { "锁库" }
                    option value="11" selected[params.transaction_type.as_deref() == Some("11")] { "解锁" }
                    option value="12" selected[params.transaction_type.as_deref() == Some("12")] { "报废" }
                }
                select class="filter-select" name="warehouse_id"
                    hx-get=(TransactionTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .txn-log-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部仓库" }
                }
                input class="search-input" type="date" name="start_date"
                    style="width:160px;padding-left:12px"
                    value=(params.start_date.as_deref().unwrap_or(""))
                    hx-get=(TransactionTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .txn-log-panel"
                    hx-swap="outerHTML";
                input class="search-input" type="date" name="end_date"
                    style="width:160px;padding-left:12px"
                    value=(params.end_date.as_deref().unwrap_or(""))
                    hx-get=(TransactionTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .txn-log-panel"
                    hx-swap="outerHTML";
            }

            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" style="min-width:1360px" {
                        thead {
                            tr {
                                th { "事务类型" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "仓库" }
                                th { "储位" }
                                th class="num-right" { "数量" }
                                th { "来源类型" }
                                th { "来源单号" }
                                th { "操作员" }
                                th { "时间" }
                            }
                        }
                        tbody {
                            @for txn in &result.items {
                                (transaction_row(txn))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无事务记录"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(TransactionListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn transaction_row(txn: &abt_core::wms::inventory::model::TransactionDetailView) -> Markup {
    let label = txn_type_label(&txn.transaction_type);
    let css_class = txn_type_class(&txn.transaction_type);
    let qty = txn.quantity;
    let is_positive = qty > rust_decimal::Decimal::ZERO;

    html! {
        tr {
            td {
                span class=(format!("txn-type {css_class}")) style="display:inline-flex;align-items:center;padding:2px 10px;border-radius:var(--radius-pill);font-size:11px;font-weight:500;white-space:nowrap" {
                    (label)
                }
            }
            td class="mono" { (txn.product_code) }
            td { (txn.product_name) }
            td { (txn.warehouse_name) }
            td class="mono" { (txn.bin_code) }
            td class="num-right" {
                @if is_positive {
                    span class="qty-positive" style="color:var(--success);font-weight:600" {
                        "+" (qty)
                    }
                } @else {
                    span class="qty-negative" style="color:var(--danger);font-weight:600" {
                        (qty)
                    }
                }
            }
            td { (txn.source_type) }
            td class="mono" {
                @if txn.source_id > 0 {
                    "#" (txn.source_id)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td { "操作员#" (txn.operator_id) }
            td class="mono" { (txn.created_at.format("%Y-%m-%d %H:%M")) }
        }
    }
}

fn build_query_string(params: &TransactionLogQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(ref tt) = params.transaction_type {
        if !tt.is_empty() {
            q.push(format!("transaction_type={tt}"));
        }
    }
    if let Some(w) = params.warehouse_id {
        q.push(format!("warehouse_id={w}"));
    }
    if let Some(ref sd) = params.start_date {
        q.push(format!("start_date={sd}"));
    }
    if let Some(ref ed) = params.end_date {
        q.push(format!("end_date={ed}"));
    }
    if let Some(p) = params.page {
        if p > 1 {
            q.push(format!("page={p}"));
        }
    }
    if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) }
}
