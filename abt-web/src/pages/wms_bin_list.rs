use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::enums::BinStatus;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::wms_bin::{BinCreatePath, BinDetailPath, BinListPath, BinTablePath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BinQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WAREHOUSE", "read")]
pub async fn get_bin_list(
    _path: BinListPath,
    ctx: RequestContext,
    Query(params): Query<BinQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.warehouse_service();

    let search_params = build_search_params(&params);
    let result = svc.search_bins_with_warehouse(&service_ctx, &mut conn, search_params).await?;
    let warehouses = svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await?;

    // Build zone lookup for display
    let mut zone_map: HashMap<i64, Zone> = HashMap::new();
    let mut fetched_wh: HashSet<i64> = HashSet::new();
    for item in &result.items {
        if fetched_wh.insert(item.warehouse_id) {
            if let Ok(zs) = svc.list_zones(&service_ctx, &mut conn, item.warehouse_id).await {
                for z in zs {
                    zone_map.insert(z.id, z);
                }
            }
        }
    }

    let content = bin_list_page(&result, &params, &warehouses.items, &zone_map);
    let page_html = admin_page(
        is_htmx,
        "储位管理",
        &claims,
        "inventory",
        BinListPath::PATH,
        "库存管理",
        Some("储位管理"),
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WAREHOUSE", "read")]
pub async fn get_bin_table(
    ctx: RequestContext,
    Query(params): Query<BinQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    let search_params = build_search_params(&params);
    let result = svc.search_bins_with_warehouse(&service_ctx, &mut conn, search_params).await?;
    let warehouses = svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await?;

    // Build zone lookup for display
    let mut zone_map: HashMap<i64, Zone> = HashMap::new();
    let mut fetched_wh: HashSet<i64> = HashSet::new();
    for item in &result.items {
        if fetched_wh.insert(item.warehouse_id) {
            if let Ok(zs) = svc.list_zones(&service_ctx, &mut conn, item.warehouse_id).await {
                for z in zs {
                    zone_map.insert(z.id, z);
                }
            }
        }
    }

    Ok(Html(bin_table_fragment(&result, &params, &warehouses.items, &zone_map).into_string()))
}

// ── Helpers ──

fn build_search_params(params: &BinQueryParams) -> SearchBinsParams {
    let is_active = match params.status {
        Some(4) => Some(false),
        Some(1) | Some(2) | Some(3) => Some(true),
        _ => None,
    };
    SearchBinsParams {
        keyword: params.keyword.clone(),
        is_active,
        warehouse_id: params.warehouse_id,
        page: params.page.unwrap_or(1),
        page_size: 20,
    }
}

pub(crate) fn bin_status_label(s: &BinStatus) -> &'static str {
    match s {
        BinStatus::Empty => "空闲",
        BinStatus::Occupied => "占用",
        BinStatus::Locked => "锁定",
        BinStatus::Disabled => "停用",
    }
}

pub(crate) fn bin_status_class(s: &BinStatus) -> &'static str {
    match s {
        BinStatus::Empty => "status-completed",
        BinStatus::Occupied => "status-accepted",
        BinStatus::Locked => "status-progress",
        BinStatus::Disabled => "status-draft",
    }
}

// ── Components ──

fn bin_list_page(
    result: &abt_core::shared::types::PaginatedResult<BinWithWarehouse>,
    params: &BinQueryParams,
    warehouses: &[Warehouse],
    zones: &HashMap<i64, Zone>,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "储位管理" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(BinCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建储位"
                    }
                }
            }

            (bin_table_fragment(result, params, warehouses, zones))
        }
    }
}

fn bin_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<BinWithWarehouse>,
    params: &BinQueryParams,
    warehouses: &[Warehouse],
    zones: &HashMap<i64, Zone>,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="bin-list-panel" {
            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索储位编码/名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(BinTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .bin-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="warehouse_id"
                    hx-get=(BinTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .bin-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部仓库" }
                    @for wh in warehouses {
                        option value=(wh.id) selected[params.warehouse_id == Some(wh.id)] {
                            (wh.name)
                        }
                    }
                }
                select class="filter-select" name="status"
                    hx-get=(BinTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .bin-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部状态" }
                    option value="1" selected[params.status == Some(1)] { "空闲" }
                    option value="2" selected[params.status == Some(2)] { "占用" }
                    option value="3" selected[params.status == Some(3)] { "锁定" }
                    option value="4" selected[params.status == Some(4)] { "停用" }
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "储位编码" }
                                th { "储位名称" }
                                th { "所属仓库" }
                                th { "所属库区" }
                                th class="num-right" { "行" }
                                th class="num-right" { "列" }
                                th class="num-right" { "层" }
                                th class="num-right" { "容量上限" }
                                th { "当前状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for item in &result.items {
                                (bin_row(item, zones))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无储位数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(BinListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn bin_row(item: &BinWithWarehouse, zones: &HashMap<i64, Zone>) -> Markup {
    let detail_path = BinDetailPath { id: item.bin.id }.to_string();
    let bin = &item.bin;
    let status_label = bin_status_label(&bin.status);
    let status_class = bin_status_class(&bin.status);
    let zone_name = zones.get(&bin.zone_id).map(|z| z.name.as_str()).unwrap_or("—");

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (bin.code) }
            td onclick=(format!("location.href='{}'", detail_path)) { (bin.name) }
            td onclick=(format!("location.href='{}'", detail_path)) { (item.warehouse_name) }
            td onclick=(format!("location.href='{}'", detail_path)) { (zone_name) }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                (bin.row_no.as_deref().unwrap_or("—"))
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                (bin.column_no.as_deref().unwrap_or("—"))
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                (bin.layer_no.as_deref().unwrap_or("—"))
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(cap) = &bin.capacity_limit {
                    (cap)
                } @else {
                    "—"
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="查看详情" href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &BinQueryParams) -> String {
    let mut q = vec![];
    if let Some(kw) = &params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(w) = params.warehouse_id {
        q.push(format!("warehouse_id={w}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    q.join("&")
}
