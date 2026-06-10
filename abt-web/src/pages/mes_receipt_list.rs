use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::ReceiptStatus;
use abt_core::mes::production_receipt::{ReceiptListItem, ReceiptListFilter};
use abt_core::mes::production_receipt::ProductionReceiptService;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{ReceiptCreatePath, ReceiptListPath, ReceiptTablePath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

fn receipt_status_label(s: &ReceiptStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        ReceiptStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        ReceiptStatus::Confirmed => ("已确认", "rgba(82,196,26,0.08)", "var(--success)"),
        ReceiptStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReceiptQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

#[require_permission("MES", "read")]
pub async fn get_receipt_list(
    _path: ReceiptListPath, ctx: RequestContext, Query(params): Query<ReceiptQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("WORK_ORDER", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_receipt_service();
    let page = params.page.unwrap_or(1);
    let filter = ReceiptListFilter { keyword: params.keyword.clone() };
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;
    let content = receipt_list_page(&result, &params, can_create);
    Ok(Html(admin_page(is_htmx, "完工入库", &claims, "production", ReceiptListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

#[require_permission("MES", "read")]
pub async fn get_receipt_table(
    _path: ReceiptTablePath, ctx: RequestContext, Query(params): Query<ReceiptQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_receipt_service();
    let page = params.page.unwrap_or(1);
    let filter = ReceiptListFilter { keyword: params.keyword.clone() };
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;
    Ok(Html(receipt_table_fragment(&result, &params).into_string()))
}

fn receipt_list_page(
    result: &PaginatedResult<ReceiptListItem>,
    params: &ReceiptQueryParams,
    can_create: bool,
) -> Markup {
    html! { div {
        div class="page-header" { h1 class="page-title" { "完工入库" } div class="page-actions" {
            @if can_create {
                a class="btn btn-primary" href=(ReceiptCreatePath::PATH) { (icon::plus_icon("w-4 h-4")) "新建入库" }
            }
        }}
        (receipt_table_fragment(result, params))
    }}
}

fn receipt_table_fragment(
    result: &PaginatedResult<ReceiptListItem>,
    params: &ReceiptQueryParams,
) -> Markup {
    html! { div {
        form class="filter-bar filter-form" hx-get=(ReceiptTablePath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#receipt-data-card" hx-select="#receipt-data-card" hx-swap="outerHTML" hx-include="closest form" {
            div class="search-wrap" { (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="keyword" style="width:180px" placeholder="搜索单号…" value=(params.keyword.as_deref().unwrap_or(""));
            }
        }
        (receipt_data_card(result, params))
    }}
}

fn receipt_data_card(
    result: &PaginatedResult<ReceiptListItem>,
    params: &ReceiptQueryParams,
) -> Markup {
    let mut qs = vec![];
    if let Some(k) = &params.keyword { qs.push(format!("keyword={k}")); }
    let query = qs.join("&");

    html! {
        div class="data-card" id="receipt-data-card" {
            div class="data-card-scroll" {
                table class="data-table" { thead { tr {
                    th { "单号" } th { "工单" } th { "批次" } th { "产品" }
                    th class="num-right" { "入库数量" } th { "仓库" } th { "状态" } th { "操作" }
                }} tbody {
                    @for item in &result.items {
                        @let status = ReceiptStatus::from_i16(item.status).unwrap_or(ReceiptStatus::Draft);
                        @let (sl, sb, sc) = receipt_status_label(&status);
                        @let dp = format!("/admin/mes/receipts/{}", item.id);
                        @let wo_doc = item.work_order_doc.as_deref().unwrap_or("—");
                        @let wh_name = item.warehouse_name.as_deref().unwrap_or("—");
                        @let prod_name = item.product_name.as_deref().unwrap_or("—");
                        tr style="cursor:pointer" onclick=(format!("location.href='{}'", dp)) {
                            td class="link-cell mono" style="color:var(--accent)" { (item.doc_number) }
                            td class="mono" { (wo_doc) }
                            td { @if let Some(bid) = item.batch_id { (bid) } @else { "—" } }
                            td { (prod_name) }
                            td class="num-right mono" { (crate::utils::fmt_qty(item.received_qty)) }
                            td { (wh_name) }
                            td { span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", sb, sc)) { (sl) } }
                            td { a href=(dp) style="color:var(--accent);font-size:var(--text-xs)" { "查看" } }
                        }
                    }
                    @if result.items.is_empty() {
                        tr { td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无入库记录" } }
                    }
                }}
            }
            (pagination(ReceiptListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
