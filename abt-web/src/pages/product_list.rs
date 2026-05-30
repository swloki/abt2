use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::model::*;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::category::CategoryService;
use abt_core::master_data::category::model::{Category, CategoryQuery};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::product::{ProductCreatePath, ProductDeletePath, ProductDetailPath, ProductListPath, ProductTablePath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProductQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub category_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_product_list(
    _path: ProductListPath,
    ctx: RequestContext,
    headers: HeaderMap,
    Query(params): Query<ProductQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.product_service();
    let cat_svc = state.category_service();
    let categories = cat_svc.list(&service_ctx, &mut conn, CategoryQuery::default(), PageParams::new(1, 200)).await?;

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let content = product_list_page(&result, &params, &categories.items);
    let page_html = admin_page(
        &headers, "产品管理", &claims, "md", ProductListPath::PATH, "主数据管理", Some("产品管理"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "read")]
pub async fn get_product_table(
    ctx: RequestContext,
    Query(params): Query<ProductQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();
    let cat_svc = state.category_service();
    let categories = cat_svc.list(&service_ctx, &mut conn, CategoryQuery::default(), PageParams::new(1, 200)).await?;

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(product_table_fragment(&result, &params, &categories.items).into_string()))
}

#[require_permission("PRODUCT", "delete")]
pub async fn delete_product(
    path: ProductDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", ProductListPath::PATH)], Html(String::new())))
}

// ── Helpers ──

fn build_filter(params: &ProductQueryParams) -> ProductQuery {
    ProductQuery {
        name: params.keyword.clone(),
        code: params.keyword.clone(),
        status: params.status.and_then(ProductStatus::from_i16),
        owner_department_id: None,
        category_id: params.category_id,
    }
}

fn build_query_string(params: &ProductQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(cid) = params.category_id {
        q.push(format!("category_id={cid}"));
    }
    q.join("&")
}

// ── Components ──

fn product_list_page(
    result: &abt_core::shared::types::PaginatedResult<Product>,
    params: &ProductQueryParams,
    categories: &[Category],
) -> Markup {
    let total_count = result.total;

    html! {
        div x-data="{ createModalOpen: false }" {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "产品管理" }
                div class="page-actions" {
                    a href=(ProductCreatePath::PATH) class="btn btn-primary" {
                        (icon::plus_icon("w-4 h-4"))
                        "新建产品"
                    }
                }
            }

            // ── Stat Cards ──
            div class="customer-stats" {
                div class="stat-card" {
                    div class="stat-icon blue" {
                        (icon::box_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { (total_count) }
                        div class="stat-label" { "产品总数" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon green" {
                        (icon::check_circle_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "在用" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        (icon::circle_alert_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "停用" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon red" {
                        (icon::x_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "作废" }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (product_table_fragment(result, params, categories))
        }
    }
}

fn product_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Product>,
    params: &ProductQueryParams,
    categories: &[Category],
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "在用", count: None },
        TabItem { value: "2".into(), label: "停用", count: None },
        TabItem { value: "3".into(), label: "作废", count: None },
    ];

    html! {
        div class="customer-list-panel" {
            (status_tabs(ProductTablePath::PATH, "closest .customer-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索产品编码、产品名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(ProductTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .customer-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="status"
                    hx-get=(ProductTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .customer-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部状态" }
                    option value="1" selected[params.status == Some(1)] { "在用" }
                    option value="2" selected[params.status == Some(2)] { "停用" }
                    option value="3" selected[params.status == Some(3)] { "作废" }
                }
                select class="filter-select" name="category_id"
                    hx-get=(ProductTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .customer-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部分类" }
                    @for cat in categories {
                        option value=(cat.category_id) selected[params.category_id == Some(cat.category_id)] { (cat.category_name) }
                    }
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格型号" }
                                th { "单位" }
                                th { "获取途径" }
                                th { "归属部门" }
                                th { "状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for p in &result.items {
                                (product_row(p))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无产品数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(ProductListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn product_row(p: &Product) -> Markup {
    let detail_path = ProductDetailPath { id: p.product_id };
    let delete_path = ProductDeletePath { id: p.product_id };
    let form_id = format!("delete-product-form-{}", p.product_id);

    let (status_label, status_class) = match p.status {
        ProductStatus::Active => ("在用", "status-accepted"),
        ProductStatus::Inactive => ("停用", "status-draft"),
        ProductStatus::Obsolete => ("作废", "status-rejected"),
    };

    let spec = &p.meta.specification;
    let channel = &p.meta.acquire_channel;

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (p.product_code) }
            td onclick=(format!("location.href='{}'", detail_path)) { strong { (p.pdt_name) } }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if spec.is_empty() {
                    span style="color:var(--muted)" { "—" }
                } @else {
                    (spec)
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { (p.unit) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if channel.is_empty() {
                    span style="color:var(--muted)" { "—" }
                } @else {
                    (channel)
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
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
                        &format!("删除后无法恢复，确定要删除产品 <strong>{}</strong> 吗？", p.pdt_name),
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
