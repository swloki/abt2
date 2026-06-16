use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::cycle_count::model::*;
use abt_core::wms::cycle_count::CycleCountService;
use abt_core::wms::enums::CycleCountStatus;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_cycle_count::{
    CycleCountCreatePath, CycleCountDetailPath, CycleCountListPath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct CycleCountQueryParams {
    pub doc_number: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_cycle_count_list(
    _path: CycleCountListPath,
    ctx: RequestContext,
    Query(params): Query<CycleCountQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let can_create = ctx.has_permission("INVENTORY", "create").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.cycle_count_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);

    let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

    let content = cycle_count_list_page(&result, &params, can_create);
    let page_html = admin_page(
        is_htmx,
        "循环盘点",
        &claims,
        "inventory",
        CycleCountListPath::PATH,
        "库存管理",
        Some("循环盘点"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &CycleCountQueryParams) -> CycleCountFilter {
    CycleCountFilter {
        status: params.status.and_then(CycleCountStatus::from_i16),
        warehouse_id: params.warehouse_id,
    }
}

fn status_label(s: &CycleCountStatus) -> &'static str {
    match s {
        CycleCountStatus::Draft => "草稿",
        CycleCountStatus::Counting => "盘点中",
        CycleCountStatus::Completed => "已完成",
        CycleCountStatus::Adjusted => "已调整",
        CycleCountStatus::Cancelled => "已取消",
    }
}

fn status_class(s: &CycleCountStatus) -> &'static str {
    match s {
        CycleCountStatus::Draft => "status-draft",
        CycleCountStatus::Counting => "status-progress",
        CycleCountStatus::Completed => "status-completed",
        CycleCountStatus::Adjusted => "status-settled",
        CycleCountStatus::Cancelled => "status-cancelled",
    }
}

// ── Components ──

fn cycle_count_list_page(
    result: &abt_core::shared::types::PaginatedResult<CycleCount>,
    params: &CycleCountQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "循环盘点" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="btn btn-primary" href=(CycleCountCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建盘点"
                        }
                    }
                }
            }

            (cycle_count_table_fragment(result, params))
        }
    }
}

fn cycle_count_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<CycleCount>,
    params: &CycleCountQueryParams,
) -> Markup {
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "盘点中", count: None },
        TabItem { value: "3".into(), label: "已完成", count: None },
        TabItem { value: "4".into(), label: "已调整", count: None },
        TabItem { value: "5".into(), label: "已取消", count: None },
    ];

    html! {
        div class="cycle-count-panel" {
            (status_tabs_with_param(CycleCountListPath::PATH, "#cycle-count-data-card", "#cycle-count-filter-form", tabs, &active_value, "status"))

            form class="filter-bar filter-form" id="cycle-count-filter-form"
                hx-get=(CycleCountListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#cycle-count-data-card"
                hx-select="#cycle-count-data-card"
                hx-swap="outerHTML"
                hx-include="#cycle-count-filter-form"
                hx-push-url="true" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="doc_number"
                        placeholder="盘点单号"
                        value=(params.doc_number.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="warehouse_id" {
                    option value="" { "全部仓库" }
                }
            }

            (cycle_count_data_card(result, params))
        }
    }
}

fn cycle_count_data_card(
    result: &abt_core::shared::types::PaginatedResult<CycleCount>,
    params: &CycleCountQueryParams,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="data-card" id="cycle-count-data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "盘点单号" }
                            th { "盘点仓库" }
                            th { "盘点库区" }
                            th { "盘点日期" }
                            th { "状态" }
                            th { "盲盘" }
                            th class="num-right" { "物料项数" }
                            th { "操作员" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            (cycle_count_row(item))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无盘点数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(CycleCountListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}

fn cycle_count_row(item: &CycleCount) -> Markup {
    let detail_path = CycleCountDetailPath { id: item.id }.to_string();
    let sl = status_label(&item.status);
    let sc = status_class(&item.status);

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) {
                (item.doc_number)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                "仓库#" (item.warehouse_id)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(zid) = item.zone_id {
                    "库区#" (zid)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                (item.count_date.format("%Y-%m-%d"))
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {sc}")) { (sl) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if item.is_blind { "是" } @else { "否" }
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                "操作员#" (item.operator_id)
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

fn build_query_string(params: &CycleCountQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.doc_number {
        q.push(format!("doc_number={v}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(w) = params.warehouse_id {
        q.push(format!("warehouse_id={w}"));
    }
    if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) }
}
