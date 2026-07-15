use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::inventory::model::TransactionLogFilter;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::enums::TransactionType;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::wms_transaction_log::TransactionListPath;
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TransactionLogQueryParams {
 pub doc_number: Option<String>,
 pub product: Option<String>,
 pub transaction_type: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 pub start_date: Option<String>,
 pub end_date: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_transaction_list(
 _path: TransactionListPath,
 ctx: RequestContext,
 Query(params): Query<TransactionLogQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
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
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &TransactionLogQueryParams) -> TransactionLogFilter {
 TransactionLogFilter {
 product_id: None,
 product_name: None,
 product_code: params.product.clone(),
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
 TransactionType::RoutingOutput => "工序产出",
 }
}

fn txn_type_class(t: &TransactionType) -> &'static str {
 match t {
 TransactionType::PurchaseReceipt
 | TransactionType::ProductionReceipt
 | TransactionType::MaterialReturn
 | TransactionType::Unlock
 | TransactionType::RoutingOutput => "txn-type-in",
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
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "库存事务日志" }
            div class="flex gap-3" {
                span
                    class="inline-flex items-center gap-2 text-xs text-muted bg-surface border border-border-soft rounded-md px-3 py-1" {}
            }
        }

        (transaction_filter_form(params))
        (transaction_data_card(result))
    }
}
}

fn transaction_filter_form(params: &TransactionLogQueryParams) -> Markup {
 html! {
    form
        class="flex items-center gap-3 mb-6 flex-wrap"
        id="transaction-filter-form"
        hx-get=(TransactionListPath::PATH)
        hx-trigger="change,keyup changed delay:300ms from:.search-input"
        hx-target="#transaction-data-card"
        hx-select="#transaction-data-card"
        hx-swap="outerHTML"
        hx-include="#transaction-filter-form"
       
    {
        div class="relative w-60 icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
        {
            (icon::search_icon(""))
            input
                class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                type="text"
                name="doc_number"
                placeholder="搜索单号…"
                value=(params.doc_number.as_deref().unwrap_or(""));
        }
        div class="relative w-40 icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
        {
            (icon::search_icon(""))
            input
                class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                type="text"
                name="product"
                placeholder="产品编码/名称"
                value=(params.product.as_deref().unwrap_or(""));
        }
        select
            class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
            name="transaction_type"
        {
            option
                value=""
                selected[
                    params.transaction_type.is_none()
                        || params.transaction_type.as_deref() == Some("")
                ]
            { "全部类型" }
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
        select
            class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
            name="warehouse_id"
        {
            option value="" { "全部仓库" }
        }
        input
            class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
            type="date"
            name="start_date"
            class="w-[140px]"
            value=(params.start_date.as_deref().unwrap_or(""));
        span class="text-muted leading-9" { "~" }
        input
            class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
            type="date"
            name="end_date"
            class="w-[140px]"
            value=(params.end_date.as_deref().unwrap_or(""));
    }
}
}

/// The data-card with table + pagination. This is the HTMX swap target.
fn transaction_data_card(
 result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory::model::TransactionDetailView>,
) -> Markup {
 html! {
    div id="transaction-data-card" class="data-card" {
        div class="overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th { "事务类型" }
                        th { "产品编码" }
                        th { "产品名称" }
                        th { "仓库" }
                        th { "库位" }
                        th class="text-right text-[13px]" { "数量" }
                        th { "来源类型" }
                        th { "来源单号" }
                        th { "操作员" }
                        th { "时间" }
                    }
                }
                tbody {
                    @for txn in &result.items { (transaction_row(txn)) }
                    @if result.items.is_empty() {
                        tr {
                            td colspan="10" class="text-center text-muted py-8" { "暂无事务记录" }
                        }
                    }
                }
            }
        }
        ({
            pagination(
                TransactionListPath::PATH,
                "#transaction-data-card",
                "#transaction-filter-form",
                result.total,
                result.page,
                result.total_pages,
            )
        })
    }
}
}

fn transaction_row(txn: &abt_core::wms::inventory::model::TransactionDetailView) -> Markup {
 let label = txn_type_label(&txn.transaction_type);
 let css_class = txn_type_class(&txn.transaction_type);
 let qty = txn.quantity;
 let is_positive = qty > rust_decimal::Decimal::ZERO;
 let qty_fmt = format!("{:.2}", qty.abs());
 let source_label = source_type_label(&txn.source_type);

 html! {
    tr {
        td {
            span
                class=({
                    format!(
                        "inline-flex items-center rounded-full text-[11px] font-medium px-2.5 py-0.5 whitespace-nowrap {}",
                        css_class,
                    )
                })
            { (label) }
        }
        td class="font-mono tabular-nums" { (txn.product_code) }
        td { (txn.product_name) }
        td { (txn.warehouse_name) }
        td class="font-mono tabular-nums" { (txn.bin_code) }
        td class="text-right text-[13px]" {
            @if is_positive {
                span class="font-semibold text-success" { "+" (qty_fmt) }
            } @else {
                span class="font-semibold text-danger" { "-" (qty_fmt) }
            }
        }
        td { (source_label) }
        td class="font-mono tabular-nums" {
            @if txn.source_id > 0 { "#" (txn.source_id) } @else {
                span class="text-muted" { "—" }
            }
        }
        td { (txn.operator_name) }
        td class="font-mono tabular-nums" { (txn.created_at.format("%Y-%m-%d %H:%M")) }
    }
}
}

fn source_type_label(s: &str) -> &str {
 match s {
 "manual" => "手工录入",
 "purchase" => "采购",
 "sales" => "销售",
 "production" => "生产",
 "transfer" | "inventory_transfer" => "调拨",
 "conversion" | "form_conversion" => "形态转换",
 "cycle_count" | "adjustment" => "盘点调整",
 "lock" => "锁库",
 "unlock" => "解锁",
 "backflush" => "倒冲",
 "requisition" => "领料",
 "arrival" => "来料",
 "scrap" => "报废",
 _ => s,
 }
}
