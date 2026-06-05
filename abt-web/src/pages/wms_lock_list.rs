use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::inventory_lock::model::*;
use abt_core::wms::inventory_lock::InventoryLockService;
use abt_core::wms::enums::LockStatus;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_inventory_lock::{
    LockCreatePath, LockDetailPath, LockListPath, LockTablePath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LockQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_lock_list(
    _path: LockListPath,
    ctx: RequestContext,
    Query(params): Query<LockQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inventory_lock_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

    let content = lock_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "库存锁定",
        &claims,
        "inventory",
        LockListPath::PATH,
        "库存管理",
        Some("库存锁定"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "read")]
pub async fn get_lock_table(
    _path: LockTablePath,
    ctx: RequestContext,
    Query(params): Query<LockQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_lock_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

    Ok(Html(lock_table_fragment(&result, &params).into_string()))
}

// ── Helpers ──

fn build_filter(params: &LockQueryParams) -> LockFilter {
    LockFilter {
        status: params.status.and_then(LockStatus::from_i16),
        warehouse_id: params.warehouse_id,
        product_id: None,
        customer_id: None,
    }
}

fn status_label(s: &LockStatus) -> &'static str {
    match s {
        LockStatus::Active => "生效",
        LockStatus::Released => "已释放",
        LockStatus::Cancelled => "已作废",
    }
}

fn status_class(s: &LockStatus) -> &'static str {
    match s {
        LockStatus::Active => "status-progress",
        LockStatus::Released => "status-completed",
        LockStatus::Cancelled => "status-cancelled",
    }
}

// ── Components ──

fn lock_list_page(
    result: &abt_core::shared::types::PaginatedResult<InventoryLock>,
    params: &LockQueryParams,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "库存锁定" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(LockCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建锁库"
                    }
                }
            }

            (lock_table_fragment(result, params))
        }
    }
}

fn lock_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<InventoryLock>,
    params: &LockQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "生效", count: None },
        TabItem { value: "2".into(), label: "已释放", count: None },
        TabItem { value: "3".into(), label: "已作废", count: None },
    ];

    html! {
        div class="lock-list-panel" {
            (status_tabs(LockTablePath::PATH, "closest .lock-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索锁库单号/产品…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(LockTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .lock-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="warehouse_id"
                    hx-get=(LockTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .lock-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部仓库" }
                }
            }

            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" style="min-width:1060px" {
                        thead {
                            tr {
                                th { "锁库单号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "锁定仓库" }
                                th class="num-right" { "锁定数量" }
                                th { "锁定原因" }
                                th { "关联客户" }
                                th { "状态" }
                                th { "操作员" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for lock in &result.items {
                                (lock_row(lock))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无锁库数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(LockListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn lock_row(lock: &InventoryLock) -> Markup {
    let detail_path = LockDetailPath { id: lock.id }.to_string();
    let sl = status_label(&lock.status);
    let sc = status_class(&lock.status);

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) {
                (lock.doc_number)
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                "产品#" (lock.product_id)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                "仓库#" (lock.warehouse_id)
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                (lock.locked_qty)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                (lock.lock_reason)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(cid) = lock.customer_id {
                    "客户#" (cid)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {sc}")) { (sl) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                "操作员#" (lock.operator_id)
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="查看" href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &LockQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(w) = params.warehouse_id {
        q.push(format!("warehouse_id={w}"));
    }
    if let Some(p) = params.page {
        if p > 1 {
            q.push(format!("page={p}"));
        }
    }
    if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) }
}
