use axum::extract::{Query, Form};
use axum_extra::routing::TypedPath;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::product::model::*;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::category::CategoryService;
use abt_core::master_data::category::model::{Category, CategoryQuery};
use abt_core::master_data::price::ProductPriceService;
use abt_core::master_data::price::model::{PriceType, PriceQuery, PriceLogEntry};
use abt_core::master_data::product_watcher::ProductWatcherService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::modal;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::product::{
    ProductCreatePath, ProductDeletePath, ProductDetailPath, ProductListPath,
    ProductTablePath, ProductUsagePath, ProductPricePath, ProductPriceHistoryPath,
    ProductWatchPath, ProductUnwatchPath,
};
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

#[derive(Debug, Deserialize)]
pub struct PriceForm {
    pub price_type: Option<String>,
    pub new_price: String,
    pub remark: Option<String>,
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
    let watcher_svc = state.product_watcher_service();
    let categories = cat_svc.list(&service_ctx, &mut conn, CategoryQuery::default(), PageParams::new(1, 200)).await?;

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    // Load watched product IDs for the current user
    let watched = watcher_svc.list_watched_products(&service_ctx, &mut conn, 1, 1000).await?;
    let watched_ids: Vec<i64> = watched.items.iter().map(|w| w.product_id).collect();

    let content = product_list_page(&result, &params, &categories.items, &watched_ids);
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
    let watcher_svc = state.product_watcher_service();
    let categories = cat_svc.list(&service_ctx, &mut conn, CategoryQuery::default(), PageParams::new(1, 200)).await?;

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let watched = watcher_svc.list_watched_products(&service_ctx, &mut conn, 1, 1000).await?;
    let watched_ids: Vec<i64> = watched.items.iter().map(|w| w.product_id).collect();

    Ok(Html(product_table_fragment(&result, &params, &categories.items, &watched_ids).into_string()))
}

#[require_permission("PRODUCT", "read")]
pub async fn get_product_usage(
    path: ProductUsagePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    let usage = svc.check_product_usage(
        &service_ctx,
        &mut conn,
        path.id,
        UsageQuery { page: 1, page_size: 50 },
    ).await?;

    Ok(Html(usage_table_fragment(path.id, &usage.items).into_string()))
}

#[require_permission("PRODUCT", "update")]
pub async fn update_product_price(
    path: ProductPricePath,
    ctx: RequestContext,
    Form(form): Form<PriceForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_price_service();

    let price_type = form.price_type
        .and_then(|s| s.parse::<i16>().ok())
        .and_then(PriceType::from_i16)
        .unwrap_or(PriceType::Purchase);

    let new_price: Decimal = form.new_price.parse()
        .map_err(|_| abt_core::shared::types::DomainError::Validation("价格格式无效".into()))?;

    svc.update_price(
        &service_ctx,
        &mut conn,
        path.id,
        price_type,
        new_price,
        form.remark.unwrap_or_default(),
    ).await?;

    let redirect = ProductDetailPath { id: path.id };
    Ok(([("HX-Redirect", redirect.to_string())], Html(String::new())))
}

#[require_permission("PRODUCT", "read")]
pub async fn get_price_history(
    path: ProductPriceHistoryPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_price_service();

    let query = PriceQuery {
        product_id: Some(path.id),
        price_type: None,
    };
    let result = svc.list_price_history(
        &service_ctx,
        &mut conn,
        query,
        PageParams::new(1, 50),
    ).await?;

    Ok(Html(price_history_table(path.id, &result.items).into_string()))
}

#[require_permission("PRODUCT", "update")]
pub async fn watch_product(
    path: ProductWatchPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_watcher_service();

    svc.watch_product(&service_ctx, &mut conn, path.id, None).await?;

    Ok((axum::http::StatusCode::OK, Html(String::new())))
}

#[require_permission("PRODUCT", "update")]
pub async fn unwatch_product(
    path: ProductUnwatchPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_watcher_service();

    svc.unwatch_product(&service_ctx, &mut conn, path.id).await?;

    Ok((axum::http::StatusCode::OK, Html(String::new())))
}

#[require_permission("PRODUCT", "delete")]
pub async fn delete_product(
    path: ProductDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    // Check usage before deleting
    let usage = svc.check_product_usage(
        &service_ctx,
        &mut conn,
        path.id,
        UsageQuery { page: 1, page_size: 1 },
    ).await?;

    if usage.total > 0 {
        let product = svc.get(&service_ctx, &mut conn, path.id).await?;
        let html = usage_error_dialog(&product.pdt_name, usage.total);
        return Ok(Html(html.into_string()).into_response());
    }

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", ProductListPath::PATH)], Html(String::new())).into_response())
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
    watched_ids: &[i64],
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
            (product_table_fragment(result, params, categories, watched_ids))
        }
    }
}

fn product_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Product>,
    params: &ProductQueryParams,
    categories: &[Category],
    watched_ids: &[i64],
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
                                (product_row(p, watched_ids))
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

fn product_row(p: &Product, watched_ids: &[i64]) -> Markup {
    let detail_path = ProductDetailPath { id: p.product_id };
    let delete_path = ProductDeletePath { id: p.product_id };
    let usage_path = ProductUsagePath { id: p.product_id };
    let price_path = ProductPricePath { id: p.product_id };
    let price_history_path = ProductPriceHistoryPath { id: p.product_id };
    let watch_path = ProductWatchPath { id: p.product_id };
    let unwatch_path = ProductUnwatchPath { id: p.product_id };
    let delete_form_id = format!("delete-product-form-{}", p.product_id);
    let price_form_id = format!("price-form-{}", p.product_id);
    let is_watched = watched_ids.contains(&p.product_id);

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
                div class="row-actions" x-data="{ deleteOpen: false, priceModalOpen: false }" {
                    a class="row-action-btn" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    // BOM usage button — loads usage into modal
                    button type="button" class="row-action-btn" title="BOM引用"
                        hx-get=(usage_path)
                        hx-target="#modal-content"
                        hx-swap="innerHTML"
                        x-on:click="document.querySelector('#modal-content').dispatchEvent(new Event('open-modal'))" {
                        (icon::link_icon("w-4 h-4"))
                    }
                    // Price setting button
                    button type="button" class="row-action-btn" title="价格设置"
                        x-on:click="priceModalOpen = true" {
                        (icon::trending_up_icon("w-4 h-4"))
                    }
                    // Watch/unwatch toggle
                    @if is_watched {
                        button type="button" class="row-action-btn" title="取消关注"
                            hx-post=(unwatch_path)
                            hx-swap="none" {
                            (icon::bell_icon("w-4 h-4"))
                        }
                    } @else {
                        button type="button" class="row-action-btn" title="关注"
                            hx-post=(watch_path)
                            hx-swap="none" {
                            (icon::bell_icon("w-4 h-4"))
                        }
                    }
                    // Delete button
                    button type="button" class="row-action-btn text-danger" title="删除"
                        x-on:click="deleteOpen = true" {
                        (icon::trash_icon("w-4 h-4"))
                    }

                    // Price modal
                    (modal::modal(
                        "priceModalOpen",
                        "设置价格",
                        "确认",
                        &price_form_id,
                        html! {
                            form id=(price_form_id) hx-post=(price_path) hx-target="closest tr" {
                                div class="form-group" {
                                    label class="form-label" { "价格类型" }
                                    select class="form-select" name="price_type" {
                                        option value="1" selected { "采购价" }
                                        option value="2" { "销售价" }
                                        option value="3" { "标准成本" }
                                    }
                                }
                                div class="form-group" {
                                    label class="form-label" { "新价格" }
                                    input class="form-input" type="text" name="new_price" required {}
                                }
                                div class="form-group" {
                                    label class="form-label" { "备注" }
                                    input class="form-input" type="text" name="remark" {}
                                }
                                div style="margin-top:var(--space-2);text-align:right" {
                                    button type="button" class="link-btn" style="font-size:13px;color:var(--primary);background:none;border:none;cursor:pointer"
                                        hx-get=(price_history_path)
                                        hx-target="#modal-content"
                                        hx-swap="innerHTML" {
                                        "查看价格变更记录 →"
                                    }
                                }
                            }
                        },
                    ))

                    // Delete confirm dialog
                    (crate::components::confirm_dialog::confirm_dialog(
                        "deleteOpen",
                        "确认删除",
                        &format!("删除后无法恢复，确定要删除产品 <strong>{}</strong> 吗？", p.pdt_name),
                        "确认删除",
                        &delete_form_id,
                        html! {
                            form id=(delete_form_id) style="display:none"
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

// ── Fragment Components ──

fn usage_table_fragment(product_id: i64, entries: &[UsageEntry]) -> Markup {
    html! {
        div class="modal-overlay is-open" x-data="{ open: true }"
            x-bind:class="{ 'is-open': open }"
            x-on:click="open = false" {
            div class="modal" x-on:click="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { "BOM 引用" }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        x-on:click="open = false" { "×" }
                }
                div style="padding:var(--space-4)" {
                    @if entries.is_empty() {
                        p style="color:var(--muted);text-align:center;padding:var(--space-4)" { "该产品未被任何 BOM 引用" }
                    } @else {
                        table class="data-table" style="width:100%" {
                            thead {
                                tr {
                                    th { "来源类型" }
                                    th { "来源编号" }
                                    th { "来源名称" }
                                }
                            }
                            tbody {
                                @for entry in entries {
                                    tr {
                                        td { (entry.source_type) }
                                        td {
                                            a href=(format!("/admin/md/boms/{}", entry.source_id)) style="color:var(--primary)" {
                                                (entry.source_id)
                                            }
                                        }
                                        td { (entry.source_name) }
                                    }
                                }
                            }
                        }
                    }
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        x-on:click="open = false" { "关闭" }
                }
            }
        }
    }
}

fn usage_error_dialog(name: &str, total: u64) -> Markup {
    html! {
        div class="dialog-overlay open" x-data="{ open: true }"
            x-bind:class="{ 'open': open }"
            x-on:click="open = false" {
            div class="dialog" x-on:click="event.stopPropagation()" {
                div class="dialog-body" {
                    div class="dialog-icon-wrap" {
                        (icon::circle_alert_icon("w-7 h-7"))
                    }
                    div class="dialog-title" { "无法删除" }
                    p class="dialog-desc" {
                        (maud::PreEscaped(format!(
                            "产品 <strong>{name}</strong> 正被 <strong>{total}</strong> 个 BOM 引用，无法删除。请先移除相关引用后再试。",
                        )))
                    }
                }
                div class="dialog-foot" {
                    button type="button" class="btn btn-primary"
                        x-on:click="open = false" { "知道了" }
                }
            }
        }
    }
}

fn price_history_table(_product_id: i64, entries: &[PriceLogEntry]) -> Markup {
    html! {
        div class="modal-overlay is-open" x-data="{ open: true }"
            x-bind:class="{ 'is-open': open }"
            x-on:click="open = false" {
            div class="modal" x-on:click="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { "价格变更记录" }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        x-on:click="open = false" { "×" }
                }
                div style="padding:var(--space-4)" {
                    @if entries.is_empty() {
                        p style="color:var(--muted);text-align:center;padding:var(--space-4)" { "暂无价格变更记录" }
                    } @else {
                        table class="data-table" style="width:100%" {
                            thead {
                                tr {
                                    th { "价格类型" }
                                    th { "原价格" }
                                    th { "新价格" }
                                    th { "备注" }
                                    th { "时间" }
                                }
                            }
                            tbody {
                                @for entry in entries {
                                    tr {
                                        td { (price_type_label(entry.price_type)) }
                                        td { (entry.old_price.map(|p| p.to_string()).unwrap_or_else(|| "—".into())) }
                                        td { (entry.new_price) }
                                        td {
                                            @if entry.remark.is_empty() {
                                                span style="color:var(--muted)" { "—" }
                                            } @else {
                                                (entry.remark)
                                            }
                                        }
                                        td { (entry.created_at.format("%Y-%m-%d %H:%M")) }
                                    }
                                }
                            }
                        }
                    }
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        x-on:click="open = false" { "关闭" }
                }
            }
        }
    }
}

fn price_type_label(pt: PriceType) -> &'static str {
    match pt {
        PriceType::Purchase => "采购价",
        PriceType::Sales => "销售价",
        PriceType::StandardCost => "标准成本",
    }
}
