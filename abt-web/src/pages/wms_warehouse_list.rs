use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::enums::{WarehouseStatus, WarehouseType};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_warehouse::{
    WarehouseCreatePath, WarehouseDeletePath, WarehouseDetailPath, WarehouseEditPath,
    WarehouseListPath, WarehouseTablePath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WarehouseQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub code: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_type: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WAREHOUSE", "read")]
pub async fn get_warehouse_list(
    _path: WarehouseListPath,
    ctx: RequestContext,
    Query(params): Query<WarehouseQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.warehouse_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

    let content = warehouse_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "仓库管理",
        &claims,
        "inventory",
        WarehouseListPath::PATH,
        "库存管理",
        Some("仓库管理"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WAREHOUSE", "read")]
pub async fn get_warehouse_table(
    ctx: RequestContext,
    Query(params): Query<WarehouseQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

    Ok(Html(warehouse_data_card(&result, &build_query_string(&params)).into_string()))
}

// ── Helpers ──

fn build_filter(params: &WarehouseQueryParams) -> WarehouseFilter {
    let keyword = match (&params.code, &params.name) {
        (Some(c), Some(n)) if !c.is_empty() && !n.is_empty() => Some(format!("{} {}", c, n)),
        (Some(c), _) if !c.is_empty() => Some(c.clone()),
        (_, Some(n)) if !n.is_empty() => Some(n.clone()),
        _ => None,
    };
    WarehouseFilter {
        warehouse_type: params.warehouse_type.and_then(WarehouseType::from_i16),
        status: params.status.and_then(WarehouseStatus::from_i16),
        keyword,
    }
}

fn warehouse_type_label(t: &WarehouseType) -> &'static str {
    match t {
        WarehouseType::RawMaterial => "原材料仓",
        WarehouseType::FinishedGoods => "成品仓",
        WarehouseType::SemiFinished => "半成品仓",
        WarehouseType::Consumable => "辅料仓",
        WarehouseType::VirtualOutsource => "虚拟仓",
    }
}

fn warehouse_status_label(s: &WarehouseStatus) -> &'static str {
    match s {
        WarehouseStatus::Active => "启用",
        WarehouseStatus::Inactive => "停用",
    }
}

fn warehouse_status_class(s: &WarehouseStatus) -> &'static str {
    match s {
        WarehouseStatus::Active => "status-accepted",
        WarehouseStatus::Inactive => "status-rejected",
    }
}

// ── Components ──

fn warehouse_list_page(
    result: &abt_core::shared::types::PaginatedResult<Warehouse>,
    params: &WarehouseQueryParams,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "仓库管理" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(WarehouseCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建仓库"
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (warehouse_table_fragment(result, params))
        }
    }
}

fn warehouse_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Warehouse>,
    params: &WarehouseQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "启用", count: None },
        TabItem { value: "2".into(), label: "停用", count: None },
    ];

    html! {
        div class="warehouse-list-panel" {
            (status_tabs(WarehouseTablePath::PATH, "closest .warehouse-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            form class="filter-bar filter-form"
                hx-get=(WarehouseTablePath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#warehouse-data-card"
                hx-select="#warehouse-data-card"
                hx-swap="outerHTML"
                hx-include="closest form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="code"
                        style="width:180px"
                        placeholder="仓库编码"
                        value=(params.code.as_deref().unwrap_or(""));
                }
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="name"
                        placeholder="仓库名称"
                        value=(params.name.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="warehouse_type" {
                    option value="" { "全部类型" }
                    option value="1" selected[params.warehouse_type == Some(1)] { "原材料仓" }
                    option value="2" selected[params.warehouse_type == Some(2)] { "成品仓" }
                    option value="3" selected[params.warehouse_type == Some(3)] { "半成品仓" }
                    option value="4" selected[params.warehouse_type == Some(4)] { "辅料仓" }
                    option value="5" selected[params.warehouse_type == Some(5)] { "虚拟仓" }
                }
            }

            (warehouse_data_card(result, &query))
        }
    }
}

fn warehouse_data_card(
    result: &abt_core::shared::types::PaginatedResult<Warehouse>,
    query: &str,
) -> Markup {
    html! {
        div id="warehouse-data-card" class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "仓库编码" }
                            th { "仓库名称" }
                            th { "仓库类型" }
                            th { "状态" }
                            th { "地址" }
                            th { "管理员" }
                            th { "库区数" }
                            th { "储位数" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for w in &result.items {
                            (warehouse_row(w))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无仓库数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(WarehouseListPath::PATH, query, result.total, result.page, result.total_pages))
        }
    }
}

fn warehouse_row(w: &Warehouse) -> Markup {
    let detail_path = WarehouseDetailPath { id: w.id }.to_string();
    let edit_path = WarehouseEditPath { id: w.id }.to_string();
    let delete_path = WarehouseDeletePath { id: w.id };

    let type_label = warehouse_type_label(&w.warehouse_type);
    let status_label = warehouse_status_label(&w.status);
    let status_class = warehouse_status_class(&w.status);

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (w.code) }
            td onclick=(format!("location.href='{}'", detail_path)) { strong { (w.name) } }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class="tag-chip tag-normal" { (type_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if w.is_virtual {
                    span style="color:var(--muted)" { "—" }
                } @else if let Some(ref addr) = w.address {
                    (addr)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="编辑" href=(edit_path) {
                        (icon::edit_icon("w-4 h-4"))
                    }
                    button type="button" class="row-action-btn text-danger" title="删除"
                        hx-post=(delete_path)
                        hx-confirm=(format!("删除后无法恢复，确定要删除仓库 <strong>{}</strong> 吗？", w.name))
                        hx-target="closest tr"
                        hx-swap="outerHTML swap:0.5s" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &WarehouseQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.code {
        q.push(format!("code={v}"));
    }
    if let Some(ref v) = params.name {
        q.push(format!("name={v}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(t) = params.warehouse_type {
        q.push(format!("warehouse_type={t}"));
    }
    q.join("&")
}
