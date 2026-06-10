use axum::extract::Query;
use axum::response::Html;
use maud::{html, Markup};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use abt_core::wms::enums::TransferStatus;
use abt_core::wms::transfer::{InventoryTransfer, TransferService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_transfer::{TransferCreatePath, TransferDetailPath, TransferListPath, TransferTablePath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TransferQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub doc_number: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_transfer_list(
    _path: TransferListPath,
    ctx: RequestContext,
    Query(params): Query<TransferQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.transfer_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;

    let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

    let content = transfer_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "库存调拨",
        &claims,
        "inventory",
        TransferListPath::PATH,
        "库存管理",
        Some("库存调拨"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "read")]
pub async fn get_transfer_table(
    _path: TransferTablePath,
    ctx: RequestContext,
    Query(params): Query<TransferQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.transfer_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;

    let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

    let query = build_query_string(&params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "在途", count: None },
        TabItem { value: "3".into(), label: "已完成", count: None },
        TabItem { value: "4".into(), label: "已取消", count: None },
    ];

    let data_card = html! {
        div class="data-card" id="transfer-data-card" {
            (status_tabs(TransferTablePath::PATH, "#transfer-data-card", ".filter-bar input, .filter-bar select", tabs, &active_value))

            form class="filter-bar filter-form"
                hx-get=(TransferTablePath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#transfer-data-card"
                hx-select="#transfer-data-card"
                hx-swap="outerHTML"
                hx-include="closest form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="doc_number"
                        placeholder="调拨单号";
                }
            }

            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "调拨单号" }
                            th { "调出仓库" }
                            th { "调入仓库" }
                            th { "调拨日期" }
                            th { "状态" }
                            th class="num-right" { "物料项数" }
                            th { "操作员" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for t in &result.items {
                            (transfer_row(t))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无调拨数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(TransferListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    };

    Ok(Html(data_card.into_string()))
}

// ── Helpers ──

fn build_filter(params: &TransferQueryParams) -> abt_core::wms::transfer::TransferFilter {
    abt_core::wms::transfer::TransferFilter {
        doc_number: params.doc_number.clone(),
        status: params.status.and_then(|s| TransferStatus::from_i16(s)),
        from_warehouse_id: None,
        to_warehouse_id: None,
    }
}

// ── Components ──

fn transfer_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<InventoryTransfer>,
    params: &TransferQueryParams,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "库存调拨" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(TransferCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建调拨"
                    }
                }
            }

            (transfer_table_fragment(result, params))
        }
    }
}

fn transfer_table_fragment(
    result: &abt_core::shared::types::pagination::PaginatedResult<InventoryTransfer>,
    params: &TransferQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "在途", count: None },
        TabItem { value: "3".into(), label: "已完成", count: None },
        TabItem { value: "4".into(), label: "已取消", count: None },
    ];

    html! {
        div class="data-card" id="transfer-data-card" {
            (status_tabs(TransferTablePath::PATH, "#transfer-data-card", ".filter-bar input, .filter-bar select", tabs, &active_value))

            form class="filter-bar filter-form"
                hx-get=(TransferTablePath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#transfer-data-card"
                hx-select="#transfer-data-card"
                hx-swap="outerHTML"
                hx-include="closest form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="doc_number"
                        placeholder="调拨单号";
                }
            }

            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "调拨单号" }
                            th { "调出仓库" }
                            th { "调入仓库" }
                            th { "调拨日期" }
                            th { "状态" }
                            th class="num-right" { "物料项数" }
                            th { "操作员" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for t in &result.items {
                            (transfer_row(t))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无调拨数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(TransferListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}

fn transfer_row(t: &InventoryTransfer) -> Markup {
    let detail_path = TransferDetailPath { id: t.id };

    let (status_label, status_class) = match t.status {
        TransferStatus::Draft => ("草稿", "status-draft"),
        TransferStatus::InTransit => ("在途", "status-progress"),
        TransferStatus::Completed => ("已完成", "status-completed"),
        TransferStatus::Cancelled => ("已取消", "status-cancelled"),
    };

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (t.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) { (t.transfer_date.to_string()) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="查看" href=(detail_path.to_string()) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &TransferQueryParams) -> String {
    let mut q = vec![];
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(ref d) = params.doc_number {
        q.push(format!("doc_number={d}"));
    }
    q.join("&")
}
