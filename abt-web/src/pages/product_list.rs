use axum::extract::{Query, Form};
use axum_extra::routing::TypedPath;
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
use crate::components::import_modal::{self, ImportModalConfig};
use crate::components::export_button::{self, ExportItem};
use crate::layout::page::admin_page;
use crate::routes::product::{
    ProductCopyPath, ProductCreatePath, ProductDeletePath, ProductDetailPath, ProductEditPath,
    ProductListPath, ProductUsagePath, ProductPricePath, ProductPriceHistoryPath,
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
    Query(params): Query<ProductQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("PRODUCT", "create").await;
    let can_delete = ctx.has_permission("PRODUCT", "delete").await;
    let can_edit = ctx.has_permission("PRODUCT", "update").await;
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

    let content = product_list_page(&result, &params, &categories, &watched_ids, can_create, can_delete, can_edit);
    let page_html = admin_page(
        is_htmx, "产品管理", &claims, "md", ProductListPath::PATH, "主数据管理", Some("产品管理"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "read")]
pub async fn get_product_usage(
    path: ProductUsagePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    let product = svc.get(&service_ctx, &mut conn, path.id).await?;
    let usage = svc.check_product_usage(
        &service_ctx,
        &mut conn,
        path.id,
        UsageQuery { page: 1, page_size: 50 },
    ).await?;

    Ok(Html(bom_drawer_content(&product, &usage.items, usage.total).into_string()))
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
    can_create: bool,
    can_delete: bool,
    can_edit: bool,
) -> Markup {

    html! {
        div { (maud::PreEscaped("<script>document.body.addEventListener('closeDrawer',()=>hsRemove(null,'#price-drawer','open'))</script>"))
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "产品管理" span style="font-size:var(--text-sm);font-weight:400;color:var(--muted);margin-left:var(--space-2)" { "(" (result.total) ")" } }
                div class="page-actions" {
                    button type="button" class="btn btn-default"
                        onclick=(import_modal::import_modal_onclick(&ImportModalConfig { import_type: "product-inventory", title: "", template_columns: "" })) {
                        (icon::upload_icon("w-4 h-4"))
                        "导入"
                    }
                    (export_button::export_dropdown(&[
                        ExportItem { label: "含库存产品", export_type: "product-all" },
                        ExportItem { label: "不含价格产品", export_type: "product-without-price" },
                    ]))
                    @if can_create {
                        a href=(ProductCreatePath::PATH) class="btn btn-primary" {
                            (icon::plus_icon("w-4 h-4"))
                            "新建产品"
                        }
                    }
                }
            }

            // ── Filter + Data Table (HTMX panel) ──
            (product_table_fragment(result, params, categories, watched_ids, can_delete, can_edit))

            // ── Price Drawer (shared) ──
            (crate::components::drawer::drawer(
                "price-drawer",
                "价格设置",
                "保存价格",
                "price-drawer-form",
                html! {
                    div id="price-drawer-body" {
                        // Content loaded via HTMX
                    }
                },
            ))

            // ── BOM 引用 Drawer ──
            (crate::components::drawer::drawer_with_footer(
                "bom-drawer",
                "BOM 引用查询",
                html! {
                    div id="bom-drawer-body" {
                        // Content loaded via HTMX
                    }
                },
                html! {
                    button type="button" class="btn btn-default"
                        onclick="hsRemove(null,'#bom-drawer','open')" { "关闭" }
                    a href="/admin/md/boms/new" class="btn btn-primary" style="text-decoration:none" {
                        (icon::plus_icon("w-4 h-4"))
                        "新建 BOM"
                    }
                },
            ))

            // ── Import Modal ──
            (import_modal::import_modal(&ImportModalConfig {
                import_type: "product-inventory",
                title: "导入产品库存",
                template_columns: "新编码, 旧编码, 物料名称, 库位编码, 库存数量, 价格, 安全库存, 分类ID",
            }))

        }
    }
}

fn product_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Product>,
    params: &ProductQueryParams,
    categories: &[CategoryTree],
    watched_ids: &[i64],
    can_delete: bool,
    can_edit: bool,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="customer-list-panel" {
            // ── Filter Bar ──
            form id="filter-form" class="filter-bar filter-form"
                hx-get=(ProductListPath::PATH)
                hx-trigger="change,keyup changed delay:300ms from:.search-input"
                hx-target=".data-card"
                hx-select=".data-card"
                hx-swap="outerHTML"
                hx-include="#filter-form"
                hx-push-url="true" {
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
                                (product_row(p, watched_ids, can_delete, can_edit))
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

fn product_row(p: &Product, watched_ids: &[i64], can_delete: bool, can_edit: bool) -> Markup {
    let detail_path = ProductDetailPath { id: p.product_id };
    let delete_path = ProductDeletePath { id: p.product_id };
    let usage_path = ProductUsagePath { id: p.product_id };
    let drawer_path = ProductPriceDrawerPath { id: p.product_id };
    let watch_path = ProductWatchPath { id: p.product_id };
    let unwatch_path = ProductUnwatchPath { id: p.product_id };
    let copy_path = ProductCopyPath { id: p.product_id };
    let edit_path = ProductEditPath { id: p.product_id };
    let is_watched = watched_ids.contains(&p.product_id);
    let spec = &p.meta.specification;

    html! {
        tr id=(format!("product-row-{}", p.product_id)) style="cursor:pointer" {
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
                div class="row-actions" onclick="event.stopPropagation()" {
                    // View detail
                    a class="row-action-btn" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    // BOM usage
                    button type="button" class="row-action-btn" title="BOM引用"
                        hx-get=(usage_path)
                        hx-target="#bom-drawer-body"
                        hx-swap="innerHTML"
                        hx-on::after-request="hsAdd(null,'#bom-drawer','open')" {
                        (icon::link_icon("w-4 h-4"))
                    }
                    // More menu trigger
                    button type="button" class="row-action-btn" title="更多"
                        id=(format!("more-btn-{}", p.product_id)) {
                        (icon::dots_vertical_icon("w-4 h-4"))
                        script { (maud::PreEscaped("me().on('click', ev => { var menu = me(ev).parentElement.querySelector('.row-actions-menu'); me(menu).classToggle('is-open'); if(menu.classList.contains('is-open')) positionDropdown(me(ev), menu) })")) }
                    }
                    // Backdrop to close menu on outside click
                    div class="dropdown-backdrop"
                        onclick="this.parentElement.querySelector('.row-actions-menu').classList.remove('is-open')" {}
                    // Dropdown menu
                    div class="row-actions-menu" onclick="event.stopPropagation()" {
                        @if can_edit {
                            a href=(edit_path) {
                                (icon::edit_icon("w-4 h-4"))
                                "编辑"
                            }
                            a href=(copy_path.to_string()) {
                                (icon::copy_icon("w-4 h-4"))
                                "复制"
                            }
                        }
                        button type="button"
                            hx-get=(drawer_path)
                            hx-target="#price-drawer-body"
                            hx-swap="innerHTML"
                            onclick="hsRemoveClosest(this,'.row-actions-menu','is-open')" hx-on::after-request="hsAdd(null,'#price-drawer','open')" {
                            (icon::currency_icon("w-4 h-4"))
                            "设置价格"
                        }
                        @if is_watched {
                            button type="button"
                                hx-post=(unwatch_path)
                                hx-swap="none"
                                onclick="hsRemoveClosest(this,'.row-actions-menu','is-open')" {
                                (icon::bell_icon("w-4 h-4"))
                                "取消关注"
                            }
                        } @else {
                            button type="button"
                                hx-post=(watch_path)
                                hx-swap="none"
                                onclick="hsRemoveClosest(this,'.row-actions-menu','is-open')" {
                                (icon::bell_icon("w-4 h-4"))
                                "关注"
                            }
                        }
                        @if can_delete {
                            button type="button" class="danger"
                                hx-post=(delete_path)
                                hx-confirm=(format!("删除后无法恢复，确定要删除产品「{}」吗？", p.pdt_name))
                                hx-target=(format!("#product-row-{}", p.product_id))
                                hx-swap="outerHTML swap:0.5s"
                                onclick="hsRemoveClosest(this,'.row-actions-menu','is-open')" {
                                (icon::trash_icon("w-4 h-4"))
                                "删除"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Fragment Components ──

fn bom_drawer_content(product: &Product, entries: &[UsageEntry], total: u64) -> Markup {
    let spec = &product.meta.specification;
    let active_count = entries.iter().filter(|e| e.bom_status == Some(2)).count();
    let draft_count = total as usize - active_count;

    html! {
        // Product info card
        div class="price-product-card" {
            div class="price-product-icon" style="background:linear-gradient(135deg,#f5f0ff,#ede5ff)" {
                (icon::bolt_icon("w-5 h-5"))
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

        // Summary stats
        div class="bom-summary" {
            div class="bom-summary-item" {
                div class="bom-summary-value accent" { (total) }
                div class="bom-summary-label" { "引用 BOM 数" }
            }
            div class="bom-summary-item" {
                div class="bom-summary-value green" { (active_count) }
                div class="bom-summary-label" { "生效中" }
            }
            div class="bom-summary-item" {
                div class="bom-summary-value" { (draft_count) }
                div class="bom-summary-label" { "草稿" }
            }
        }

        // BOM list
        div class="price-section" {
            div class="price-section-title" {
                (icon::clipboard_list_icon("w-3.5 h-3.5"))
                "BOM 清单"
            }
            @if entries.is_empty() {
                div class="bom-empty" {
                    (icon::clipboard_list_icon("w-12 h-12"))
                    p { "暂无 BOM 引用" }
                    p class="sub" { "该产品尚未被任何 BOM 引用" }
                }
            } @else {
                @for entry in entries {
                    (bom_ref_card(entry))
                }
            }
        }
    }
}

fn bom_ref_card(entry: &UsageEntry) -> Markup {
    let bom_detail_path = format!("/admin/md/boms/{}", entry.source_id);
    let status = entry.bom_status.unwrap_or(1);
    let (status_label, status_class) = match status {
        2 => ("生效中", "status-completed"),
        _ => ("草稿", "status-draft"),
    };
    let version = entry.bom_version.map(|v| format!("V{}", v)).unwrap_or_else(|| "—".into());
    let qty = entry.quantity.map(|q| format!("{:.0}", q)).unwrap_or_else(|| "—".into());
    let unit = entry.node_unit.as_deref().unwrap_or("");
    let parent_name = entry.parent_product_name.as_deref().unwrap_or("");
    let has_detail = entry.bom_version.is_some() || entry.parent_product_name.is_some();

    html! {
        div class="bom-ref-card" {
            div class="bom-ref-main" {
                div class="bom-ref-icon parent" {
                    (icon::bolt_icon("w-4.5 h-4.5"))
                }
                div class="bom-ref-info" {
                    div class="bom-ref-name" {
                        a href=(bom_detail_path) onclick="event.stopPropagation()" style="color:var(--accent);text-decoration:none;font-weight:600" {
                            (entry.source_name)
                        }
                        span style="font-size:11px;font-weight:400;color:var(--muted);font-family:var(--font-mono)" {
                            "BOM-" (entry.source_id)
                        }
                    }
                    div class="bom-ref-meta" {
                        span { "版本 " (version) }
                        @if !parent_name.is_empty() && parent_name != "—" {
                            span { "父件: " (parent_name) }
                        }
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="bom-ref-right" {
                    div class="bom-ref-qty" {
                        div class="bom-ref-qty-value" {
                            (qty) " "
                            span style="font-size:12px;font-weight:400;color:var(--muted)" { (unit) }
                        }
                    }
                    button class="bom-ref-expand" {
                        svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {
                            path d="M19 9l-7 7-7-7" {}
                        }
                        script { (maud::PreEscaped("me().on('click', ev => { halt(ev); var c=me(ev).closest('.bom-ref-card'); me(c).classToggle('is-expanded'); var d=me('.bom-ref-detail', c); d.styles({display: d.style.display==='none'?'':'none'}) })")) }
                    }
                }
                script { (maud::PreEscaped("me().on('click', ev => { me(me(ev).closest('.bom-ref-card')).classToggle('is-expanded'); var d=me(ev).nextElementSibling; d?.styles({display: d.style.display==='none'?'':'none'}) })")) }
            }
            @if has_detail {
                div class="bom-ref-detail" style="display:none" {
                    div class="bom-ref-detail-grid" {
                        div class="bom-ref-detail-item" {
                            span class="label" { "BOM 编码:" }
                            span class="value" style="font-family:var(--font-mono)" { "BOM-" (entry.source_id) }
                        }
                        div class="bom-ref-detail-item" {
                            span class="label" { "版本:" }
                            span class="value" { (version) }
                        }
                        @if let Some(pn) = &entry.parent_product_name {
                            div class="bom-ref-detail-item" {
                                span class="label" { "父件产品:" }
                                span class="value" {
                                    a href=(format!("/admin/md/products/{}", entry.parent_product_code.as_deref().unwrap_or(""))) style="color:var(--accent);text-decoration:none" {
                                        (pn)
                                    }
                                }
                            }
                        }
                        @if let Some(pc) = &entry.parent_product_code {
                            div class="bom-ref-detail-item" {
                                span class="label" { "父件编码:" }
                                span class="value" style="font-family:var(--font-mono)" { (pc) }
                            }
                        }
                        div class="bom-ref-detail-item" {
                            span class="label" { "用量:" }
                            span class="value" style="font-family:var(--font-mono)" {
                                (qty) " " (unit)
                            }
                        }
                        @if let Some(remark) = &entry.node_remark {
                            @if !remark.is_empty() {
                                div class="bom-ref-detail-item" style="grid-column:1/-1" {
                                    span class="label" { "备注:" }
                                    span class="value" { (remark) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn usage_error_dialog(name: &str, total: u64) -> Markup {
    html! {
        div class="dialog-overlay open"
            onclick="hsRemove(this,null,'open')" {
            div class="dialog" onclick="event.stopPropagation()" {
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
                        onclick="hsRemoveClosest(this,'.dialog-overlay','open')" { "知道了" }
                }
            }
        }
    }
}

fn price_history_table(_product_id: i64, entries: &[PriceLogEntry]) -> Markup {
    html! {
        div class="modal-overlay is-open"
            onclick="hsRemove(this,null,'is-open')" {
            div class="modal" onclick="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { "价格变更记录" }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        onclick="hsRemoveClosest(this,'.modal-overlay','is-open')" { "×" }
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
                        onclick="hsRemoveClosest(this,'.modal-overlay','is-open')" { "关闭" }
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
    let is_up = entry.old_price.is_some_and(|old| entry.new_price >= old);
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