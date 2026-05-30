use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::bom::model::*;
use abt_core::master_data::bom::{BomCommandService, BomQueryService};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::bom::{
    BomCreatePath, BomDeletePath, BomDetailPath, BomListPath, BomTablePath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BomQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub category_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("BOM", "read")]
pub async fn get_bom_list(
    _path: BomListPath,
    ctx: RequestContext,
    headers: HeaderMap,
    Query(params): Query<BomQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.bom_query_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let content = bom_list_page(&result, &params);
    let page_html = admin_page(
        &headers, "BOM管理", &claims, "md-bom", BomListPath::PATH,
        "主数据管理", Some("BOM管理"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("BOM", "read")]
pub async fn get_bom_table(
    ctx: RequestContext,
    Query(params): Query<BomQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.bom_query_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(bom_table_fragment(&result, &params).into_string()))
}

#[require_permission("BOM", "delete")]
pub async fn delete_bom(
    path: BomDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.bom_command_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", BomListPath::PATH)], Html(String::new())))
}

// ── Helpers ──

fn build_filter(params: &BomQueryParams) -> BomQuery {
    BomQuery {
        name: params.keyword.clone(),
        status: params.status.and_then(BomStatus::from_i16),
        bom_category_id: params.category_id,
    }
}

fn build_query_string(params: &BomQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(c) = params.category_id {
        q.push(format!("category_id={c}"));
    }
    q.join("&")
}

// ── Components ──

fn bom_list_page(
    result: &abt_core::shared::types::PaginatedResult<Bom>,
    params: &BomQueryParams,
) -> Markup {
    let total_count = result.total;

    html! {
        div x-data="{ createModalOpen: false }" {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "BOM管理" }
                div class="page-actions" {
                    a href=(BomCreatePath::PATH) class="btn btn-primary" {
                        (icon::plus_icon("w-4 h-4"))
                        "新建BOM"
                    }
                }
            }

            // ── Stat Cards ──
            div class="customer-stats" {
                div class="stat-card" {
                    div class="stat-icon blue" {
                        (icon::clipboard_list_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { (total_count) }
                        div class="stat-label" { "BOM总数" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        (icon::edit_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "草稿" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon green" {
                        (icon::check_circle_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "已发布" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon purple" {
                        (icon::trending_up_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "本月新建" }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (bom_table_fragment(result, params))
        }
    }
}

fn bom_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Bom>,
    params: &BomQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已发布", count: None },
    ];

    html! {
        div class="customer-list-panel" {
            (status_tabs(BomTablePath::PATH, "closest .customer-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索BOM名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(BomTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .customer-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="status"
                    hx-get=(BomTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .customer-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部状态" }
                    option value="1" selected[params.status == Some(1)] { "草稿" }
                    option value="2" selected[params.status == Some(2)] { "已发布" }
                }
                select class="filter-select" name="category_id"
                    hx-get=(BomTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .customer-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部分类" }
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "BOM名称" }
                                th { "BOM分类" }
                                th { "版本" }
                                th { "状态" }
                                th { "发布时间" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for bom in &result.items {
                                (bom_row(bom))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无BOM数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(BomListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn bom_row(bom: &Bom) -> Markup {
    let detail_path = BomDetailPath { id: bom.bom_id };
    let delete_path = BomDeletePath { id: bom.bom_id };
    let form_id = format!("delete-bom-form-{}", bom.bom_id);

    let (status_label, status_class) = match bom.status {
        BomStatus::Draft => ("草稿", "status-draft"),
        BomStatus::Published => ("已发布", "status-accepted"),
    };

    html! {
        tr style="cursor:pointer" {
            td onclick=(format!("location.href='{}'", detail_path)) {
                strong { (bom.bom_name) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                "v"(bom.version)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(pa) = bom.published_at {
                    (pa.format("%Y-%m-%d").to_string())
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                (bom.create_at.format("%Y-%m-%d").to_string())
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" x-data="{ deleteOpen: false }" {
                    a class="row-action-btn" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    button type="button" class="row-action-btn text-danger" title="删除"
                        x-on:click="deleteOpen = true" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                    (crate::components::confirm_dialog::confirm_dialog(
                        "deleteOpen",
                        "确认删除",
                        &format!("删除后无法恢复，确定要删除BOM <strong>{}</strong> 吗？", bom.bom_name),
                        "确认删除",
                        &form_id,
                        html! {
                            form id=(form_id) style="display:none"
                                hx-post=(delete_path)
                                hx-target="closest tr"
                                hx-swap="outerHTML swap:0.5s" {}
                        },
                    ))
                }
            }
        }
    }
}
