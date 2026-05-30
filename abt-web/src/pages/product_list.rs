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
use abt_core::master_data::category::model::CategoryTree;
use abt_core::master_data::price::ProductPriceService;
use abt_core::master_data::price::model::{PriceType, PriceQuery, PriceLogEntry};
use abt_core::master_data::product_watcher::ProductWatcherService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::category_select::category_tree_select;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::product::{
    ProductCopyPath, ProductCreatePath, ProductDeletePath, ProductDetailPath, ProductListPath,
    ProductTablePath, ProductUsagePath, ProductPricePath, ProductPriceHistoryPath,
    ProductPriceDrawerPath, ProductWatchPath, ProductUnwatchPath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProductQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub code: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub name: Option<String>,
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
    let categories = cat_svc.get_tree(&service_ctx, &mut conn, None, None).await?;

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    // Load watched product IDs for the current user
    let watched = watcher_svc.list_watched_products(&service_ctx, &mut conn, 1, 1000).await?;
    let watched_ids: Vec<i64> = watched.items.iter().map(|w| w.product_id).collect();

    let content = product_list_page(&result, &params, &categories, &watched_ids);
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
    let categories = cat_svc.get_tree(&service_ctx, &mut conn, None, None).await?;

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let watched = watcher_svc.list_watched_products(&service_ctx, &mut conn, 1, 1000).await?;
    let watched_ids: Vec<i64> = watched.items.iter().map(|w| w.product_id).collect();

    Ok(Html(product_table_fragment(&result, &params, &categories, &watched_ids).into_string()))
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

    Ok(([("HX-Trigger", "{\"closeDrawer\":\"\"}")], Html(String::new())))
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
        keyword: None,
        date_from: None,
        date_to: None,
    };
    let result = svc.list_price_history(
        &service_ctx,
        &mut conn,
        query,
        PageParams::new(1, 50),
    ).await?;

    Ok(Html(price_history_table(path.id, &result.items).into_string()))
}

#[require_permission("PRODUCT", "read")]
pub async fn get_price_drawer(
    path: ProductPriceDrawerPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product_svc = state.product_service();
    let price_svc = state.product_price_service();

    let product = product_svc.get(&service_ctx, &mut conn, path.id).await?;
    let current_price = price_svc.get_current_price(&service_ctx, &mut conn, path.id, PriceType::Purchase).await?.unwrap_or_default();
    let query = PriceQuery {
        product_id: Some(path.id),
        price_type: None,
        keyword: None,
        date_from: None,
        date_to: None,
    };
    let history = price_svc.list_price_history(
        &service_ctx,
        &mut conn,
        query,
        PageParams::new(1, 3),
    ).await?;
    Ok(Html(price_drawer_content(
        &product,
        &current_price,
        &history.items,
        history.total,
    ).into_string()))
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
    let name = params.name.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    let code = params.code.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    ProductQuery {
        name,
        code,
        status: params.status.and_then(ProductStatus::from_i16),
        owner_department_id: None,
        category_id: params.category_id,
    }
}

fn build_query_string(params: &ProductQueryParams) -> String {
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
    if let Some(cid) = params.category_id {
        q.push(format!("category_id={cid}"));
    }
    q.join("&")
}

// ── Components ──

fn product_list_page(
    result: &abt_core::shared::types::PaginatedResult<Product>,
    params: &ProductQueryParams,
    categories: &[CategoryTree],
    watched_ids: &[i64],
) -> Markup {

    html! {
        div x-data="{ createModalOpen: false, priceDrawerOpen: false }" x-init="window.addEventListener('closeDrawer', () => { priceDrawerOpen = false })" {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "产品管理" span style="font-size:var(--text-sm);font-weight:400;color:var(--muted);margin-left:var(--space-2)" { "(" (result.total) ")" } }
                div class="page-actions" {
                    a href=(ProductCreatePath::PATH) class="btn btn-primary" {
                        (icon::plus_icon("w-4 h-4"))
                        "新建产品"
                    }
                }
            }

            // ── Filter + Data Table (HTMX panel) ──
            (product_table_fragment(result, params, categories, watched_ids))

            // ── Price Drawer (shared) ──
            (crate::components::drawer::drawer(
                "priceDrawerOpen",
                "价格设置",
                "保存价格",
                "price-drawer-form",
                html! {
                    div id="price-drawer-body" {
                        // Content loaded via HTMX
                    }
                },
            ))
        }
    }
}

fn product_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Product>,
    params: &ProductQueryParams,
    categories: &[CategoryTree],
    watched_ids: &[i64],
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="customer-list-panel" {
            // ── Filter Bar ──
            form class="filter-bar filter-form"
                hx-get=(ProductTablePath::PATH)
                hx-trigger="change,keyup changed delay:300ms from:.search-input"
                hx-target=".data-card"
                hx-select=".data-card"
                hx-swap="outerHTML"
                hx-include="closest form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="code"
                        style="width:180px"
                        placeholder="产品编码"
                        value=(params.code.as_deref().unwrap_or(""));
                }
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="name"
                        placeholder="产品名称"
                        value=(params.name.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="status" {
                    option value="" { "全部状态" }
                    option value="1" selected[params.status == Some(1)] { "在用" }
                    option value="2" selected[params.status == Some(2)] { "停用" }
                    option value="3" selected[params.status == Some(3)] { "作废" }
                }
                (category_tree_select(
                    categories,
                    params.category_id,
                    "category_id",
                    "全部分类",
                ))
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
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for p in &result.items {
                                (product_row(p, watched_ids))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="5" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
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
    let drawer_path = ProductPriceDrawerPath { id: p.product_id };
    let watch_path = ProductWatchPath { id: p.product_id };
    let unwatch_path = ProductUnwatchPath { id: p.product_id };
    let copy_path = ProductCopyPath { id: p.product_id };
    let edit_path = format!("/admin/md/products/{}", p.product_id);
    let delete_form_id = format!("delete-product-form-{}", p.product_id);
    let is_watched = watched_ids.contains(&p.product_id);
    let spec = &p.meta.specification;

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
            td onclick="event.stopPropagation()" {
                div class="row-actions" x-data="{ menuOpen: false, deleteOpen: false }" x-effect="if(menuOpen) $nextTick(function(){ positionDropdown($refs.moreBtn, $refs.menu) })" {
                    // View detail
                    a class="row-action-btn" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    // BOM usage
                    button type="button" class="row-action-btn" title="BOM引用"
                        hx-get=(usage_path)
                        hx-target="#modal-content"
                        hx-swap="innerHTML"
                        x-on:click="document.querySelector('#modal-content').dispatchEvent(new Event('open-modal'))" {
                        (icon::link_icon("w-4 h-4"))
                    }
                    // More menu trigger
                    button type="button" class="row-action-btn" title="更多"
                        x-ref="moreBtn"
                        x-on:click="menuOpen = !menuOpen" {
                        (icon::dots_vertical_icon("w-4 h-4"))
                    }
                    // Backdrop to close menu on outside click
                    div x-show="menuOpen" x-cloak style="position:fixed;inset:0;z-index:49" x-on:click="menuOpen = false" {}
                    // Dropdown menu (positioned by JS)
                    div x-show="menuOpen" x-cloak x-ref="menu"
                        x-transition:enter="transition ease-out duration-100"
                        x-transition:enter-start="opacity-0 scale-95"
                        x-transition:enter-end="opacity-100 scale-100"
                        x-transition:leave="transition ease-in duration-75"
                        x-transition:leave-start="opacity-100 scale-100"
                        x-transition:leave-end="opacity-0 scale-95"
                        class="row-actions-menu" {
                        a href=(edit_path) {
                            (icon::edit_icon("w-4 h-4"))
                            "编辑"
                        }
                        a href=(copy_path.to_string()) {
                            (icon::copy_icon("w-4 h-4"))
                            "复制"
                        }
                        button type="button"
                            hx-get=(drawer_path)
                            hx-target="#price-drawer-body"
                            hx-swap="innerHTML"
                            x-on:click="menuOpen = false; priceDrawerOpen = true" {
                            (icon::currency_icon("w-4 h-4"))
                            "设置价格"
                        }
                        @if is_watched {
                            button type="button"
                                hx-post=(unwatch_path)
                                hx-swap="none"
                                x-on:click="menuOpen = false" {
                                (icon::bell_icon("w-4 h-4"))
                                "取消关注"
                            }
                        } @else {
                            button type="button"
                                hx-post=(watch_path)
                                hx-swap="none"
                                x-on:click="menuOpen = false" {
                                (icon::bell_icon("w-4 h-4"))
                                "关注"
                            }
                        }
                        button type="button" class="danger"
                            x-on:click="menuOpen = false; deleteOpen = true" {
                            (icon::trash_icon("w-4 h-4"))
                            "删除"
                        }
                    }
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
                div class="modal-body" {
                    @if entries.is_empty() {
                        div class="empty-state" { "该产品未被任何 BOM 引用" }
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
                div class="modal-body" {
                    @if entries.is_empty() {
                        div class="empty-state" { "暂无价格变更记录" }
                    } @else {
                        @for entry in entries {
                            (price_history_diff_item(entry))
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

fn price_drawer_content(product: &Product, current_price: &Decimal, history: &[PriceLogEntry], total_count: u64) -> Markup {
    let price_path = ProductPricePath { id: product.product_id };
    let spec = &product.meta.specification;
    let has_more = (total_count as usize) > history.len();
    html! {
        form id="price-drawer-form" hx-post=(price_path) hx-target="#price-drawer-body" hx-swap="innerHTML" {
            input type="hidden" name="price_type" value="1";
            // Product info card
            div class="price-product-card" {
                div class="price-product-icon" {
                    (icon::box_icon("w-5 h-5"))
                }
                div style="flex:1;min-width:0" {
                    div class="price-product-name" { (product.pdt_name) }
                    div class="price-product-meta" {
                        (product.product_code) "  \u{00b7}  "
                        @if spec.is_empty() {
                            (product.unit)
                        } @else {
                            (spec) " / " (product.unit)
                        }
                    }
                }
            }
            // Price section
            div class="price-section" {
                div class="price-section-title" {
                    (icon::currency_icon("w-3.5 h-3.5"))
                    "产品单价"
                }
                div class="price-row" {
                    div class="price-row-label" { "单价" }
                    div class="prefix" { "¥" }
                    input type="text" name="new_price"
                        value=(format!("{:.4}", current_price))
                        placeholder="0.0000";
                }
            }
            // Remark section
            div class="price-section" {
                div class="price-section-title" {
                    (icon::comment_icon("w-3.5 h-3.5"))
                    "调价说明"
                }
                div class="form-field" {
                    textarea name="remark" placeholder="调价原因（如：原材料上涨、供应商调价、季度促销等）" rows="2" style="resize:none;width:100%;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-md);font-size:13px;color:var(--fg);font-family:var(--font-body)" {}
                }
            }
            // Price history
            div class="price-section" {
                div class="price-section-title" {
                    (icon::clock_icon("w-3.5 h-3.5"))
                    "变更历史"
                }
                @if history.is_empty() {
                    div style="text-align:center;padding:var(--space-4);color:var(--muted);font-size:13px" { "暂无价格变更记录" }
                } @else {
                    @for entry in history {
                        (price_history_diff_item(entry))
                    }
                    @if has_more {
                        a class="price-history-more" href="/admin/md/price-history" {
                            (icon::chevron_down_icon("w-3 h-3"))
                            "查看全部 " (total_count) " 条变更记录"
                        }
                    }
                }
            }
        }
    }
}

fn price_history_diff_item(entry: &PriceLogEntry) -> Markup {
    let old_str = entry.old_price.map(|p| format!("{:.4}", p)).unwrap_or_else(|| "—".into());
    let new_str = format!("{:.4}", entry.new_price);
    let pct = match entry.old_price {
        Some(old) if !old.is_zero() => {
            let change = (entry.new_price - old) / old * rust_decimal::Decimal::from(100);
            if change >= rust_decimal::Decimal::ZERO {
                format!("+{:.1}%", change)
            } else {
                format!("{:.1}%", change)
            }
        }
        _ => "—".into(),
    };
    let is_up = entry.old_price.map_or(false, |old| entry.new_price >= old);
    let badge_class = if is_up { "change-badge up" } else { "change-badge down" };

    html! {
        div class="price-history-item" {
            div class="price-diff" {
                span class="old-price" { "¥ " (old_str) }
                svg class="arrow-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" { path d="M17 8l4 4m0 0l-4 4m4-4H3" {} }
                span class="new-price" { "¥ " (new_str) }
                span class=(badge_class) { (pct) }
            }
            div class="meta" {
                span { (price_type_label(entry.price_type)) }
                span { (entry.created_at.format("%Y-%m-%d")) }
                @if !entry.remark.is_empty() {
                    span style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" { (entry.remark) }
                }
            }
        }
    }
}