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
 div { (maud::PreEscaped("<script>document.body.addEventListener('closeDrawer',()=>document.querySelector('#price-drawer').classList.remove('open'))</script>"))
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "产品管理" span class="text-sm font-normal text-muted ml-2" { "(" (result.total) ")" } }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _=(import_modal::import_modal_onclick(&ImportModalConfig { import_type: "product-inventory", title: "", template_columns: "" })) {
 (icon::upload_icon("w-4 h-4"))
 "导入"
 }
 (export_button::export_dropdown(&[
 ExportItem { label: "含库存产品", export_type: "product-all" },
 ExportItem { label: "不含价格产品", export_type: "product-without-price" },
 ]))
 @if can_create {
 a href=(ProductCreatePath::PATH) class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
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
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from #bom-drawer" { "关闭" }
 a href="/admin/md/boms/new" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" class="no-underline" {
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
 form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
 hx-get=(ProductListPath::PATH)
 hx-trigger="change,keyup changed delay:300ms from:.search-input"
 hx-target=".data-card"
 hx-select=".data-card"
 hx-swap="outerHTML"
 hx-include="#filter-form"
 hx-push-url="true" {
 div class="relative w-60" {
 (icon::search_icon("absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code"
 placeholder="产品编码"
 value=(params.code.as_deref().unwrap_or_default());
 }
 div class="relative w-60" {
 (icon::search_icon("absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name"
 placeholder="产品名称"
 value=(params.name.as_deref().unwrap_or_default());
 }
 select class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" name="status" {
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
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "产品编码" }
 th { "产品名称" }
 th { "规格型号" }
 th { "单位" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for p in &result.items {
 (product_row(p, watched_ids, can_delete, can_edit))
 }
 @if result.items.is_empty() {
 tr { td colspan="6" class="text-center text-muted text-sm py-8" {
 "暂无产品数据"
 } }
 }
 }
 }
 }
 }
 (pagination(ProductListPath::PATH, &query, result.total, result.page, result.total_pages))
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
 tr id=(format!("product-row-{}", p.product_id)) class="hover:bg-accent-bg transition-colors" {
 td {
 a href=(detail_path.to_string()) class="text-accent font-medium font-mono tabular-nums hover:underline" { (p.product_code) }
 }
 td { a href=(detail_path.to_string()) class="text-fg hover:text-accent font-medium" { strong { (p.pdt_name) } } }
 td class="text-sm text-muted" {
 @if spec.is_empty() {
 span class="text-muted" { "—" }
 } @else {
 (spec)
 }
 }
 td class="text-sm text-fg-2" { (p.unit) }
 @let (status_label, status_cls) = product_status_badge(&p.status);
 td { span class=(format!("status-pill {}", status_cls)) { (status_label) } }
 td {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 // View detail
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="查看"
 href=(detail_path) {
 (icon::eye_icon("w-4 h-4"))
 }
 // BOM usage
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="BOM引用"
 hx-get=(usage_path)
 hx-target="#bom-drawer-body"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .open to #bom-drawer" {
 (icon::link_icon("w-4 h-4"))
 }
 // More menu trigger
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="更多"
 id=(format!("more-btn-{}", p.product_id))
 _="on click if next .row-actions-menu's style's display is 'none'
 then show next .row-actions-menu then show next .row-actions-menu-backdrop then call positionDropdown(me, next .row-actions-menu)
 else hide next .row-actions-menu then hide next .row-actions-menu-backdrop" {
 (icon::dots_vertical_icon("w-4 h-4"))
 }
 div class="row-actions-menu-backdrop fixed inset-0 z-[999] cursor-default" style="display:none"
 _="on click remove .is-open from next .row-actions-menu then hide next .row-actions-menu then hide me" {}
 div class="row-actions-menu fixed z-[50] bg-bg border border-border rounded shadow-[var(--shadow-card)] min-w-[140px] py-1" style="display:none" {
 @if can_edit {
 a href=(edit_path) class="flex items-center gap-2 w-full text-left px-4 py-2 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors no-underline" {
 (icon::edit_icon("w-4 h-4"))
 "编辑"
 }
 a href=(copy_path.to_string()) class="flex items-center gap-2 w-full text-left px-4 py-2 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors no-underline" {
 (icon::copy_icon("w-4 h-4"))
 "复制"
 }
 }
 button type="button" class="flex items-center gap-2 w-full text-left px-4 py-2 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors border-none bg-transparent cursor-pointer"
 hx-get=(drawer_path)
 hx-target="#price-drawer-body"
 hx-swap="innerHTML"
 _="on click hide closest .row-actions-menu then hide closest .row-actions-menu-backdrop then remove .is-open from closest .row-actions-menu on 'htmx:afterRequest' add .open to #price-drawer" {
 (icon::currency_icon("w-4 h-4"))
 "设置价格"
 }
 @if is_watched {
 button type="button" class="flex items-center gap-2 w-full text-left px-4 py-2 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors border-none bg-transparent cursor-pointer"
 hx-post=(unwatch_path)
 hx-swap="none"
 _="on click hide closest .row-actions-menu then hide closest .row-actions-menu-backdrop then remove .is-open from closest .row-actions-menu" {
 (icon::bell_icon("w-4 h-4"))
 "取消关注"
 }
 } @else {
 button type="button" class="flex items-center gap-2 w-full text-left px-4 py-2 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors border-none bg-transparent cursor-pointer"
 hx-post=(watch_path)
 hx-swap="none"
 _="on click hide closest .row-actions-menu then hide closest .row-actions-menu-backdrop then remove .is-open from closest .row-actions-menu" {
 (icon::bell_icon("w-4 h-4"))
 "关注"
 }
 }
 @if can_delete {
 button type="button" class="flex items-center gap-2 w-full text-left px-4 py-2 text-sm text-danger hover:bg-[rgba(220,38,38,0.08)] transition-colors border-none bg-transparent cursor-pointer"
 hx-post=(delete_path)
 hx-confirm=(format!("删除后无法恢复，确定要删除产品「{}」吗？", p.pdt_name))
 hx-target=(format!("#product-row-{}", p.product_id))
 hx-swap="outerHTML swap:0.5s"
 _="on click hide closest .row-actions-menu then hide closest .row-actions-menu-backdrop then remove .is-open from closest .row-actions-menu" {
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

fn product_status_badge(s: &ProductStatus) -> (&'static str, &'static str) {
 match s {
 ProductStatus::Active => ("在用", "status-completed"),
 ProductStatus::Inactive => ("停用", "status-warn"),
 ProductStatus::Obsolete => ("作废", "status-cancelled"),
 }
}

// ── Fragment Components ──

fn bom_drawer_content(product: &Product, entries: &[UsageEntry], total: u64) -> Markup {
 let spec = &product.meta.specification;
 let active_count = entries.iter().filter(|e| e.bom_status == Some(2)).count();
 let draft_count = total as usize - active_count;

 html! {
 // Product info card
 div class="flex items-start gap-[14px] bg-surface rounded-lg" {
 div class="w-[40px] h-[40px] rounded flex items-center justify-center shrink-0" style="background:linear-gradient(135deg,#f5f0ff,#ede5ff)" {
 (icon::bolt_icon("w-5 h-5"))
 }
 div class="flex-1 min-w-0" {
 div class="text-[15px] font-semibold text-fg" { (product.pdt_name) }
 div class="text-[12px] text-muted" {
 (product.product_code) " \u{00b7} "
 @if spec.is_empty() {
 (product.unit)
 } @else {
 (spec) " / " (product.unit)
 }
 }
 }
 }

 // Summary stats
 div class="grid gap-[12px]" {
 div class="grid gap-[12px]-item" {
 div class="grid gap-[12px]-value accent" { (total) }
 div class="grid gap-[12px]-label" { "引用 BOM 数" }
 }
 div class="grid gap-[12px]-item" {
 div class="grid gap-[12px]-value green" { (active_count) }
 div class="grid gap-[12px]-label" { "生效中" }
 }
 div class="grid gap-[12px]-item" {
 div class="grid gap-[12px]-value" { (draft_count) }
 div class="grid gap-[12px]-label" { "草稿" }
 }
 }

 // BOM list
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::clipboard_list_icon("w-3.5 h-3.5"))
 "BOM 清单"
 }
 @if entries.is_empty() {
 div class="text-center text-muted" {
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
 div class="border border-border rounded overflow-hidden" {
 div class="flex items-center gap-[14px] cursor-pointer" _="on click toggle .is-expanded on closest .bom-ref-card" {
 div class="w-[38px] h-[38px] rounded-sm shrink-0 flex items-center justify-center parent" {
 (icon::bolt_icon("w-4.5 h-4.5"))
 }
 div class="bom-ref-info" {
 div class="text-[14px] font-semibold text-fg flex items-center gap-[8px]" {
 a href=(bom_detail_path) class="text-accent no-underline font-semibold" {
 (entry.source_name)
 }
 span class="text-[11px] font-normal text-muted font-mono" {
 "BOM-" (entry.source_id)
 }
 }
 div class="flex items-center gap-[12px] text-[12px] text-muted" {
 span { "版本 " (version) }
 @if !parent_name.is_empty() && parent_name != "—" {
 span { "父件: " (parent_name) }
 }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 }
 div class="flex items-center gap-[10px] shrink-0" {
 div class="text-right" {
 div class="text-[16px] font-bold text-fg" {
 (qty) " "
 span class="text-xs font-normal text-muted" { (unit) }
 }
 }
 button class="border-none cursor-pointer text-muted flex items-center justify-center" {
 svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {
 path d="M19 9l-7 7-7-7" {}
 }
 
 }
 }

 }
 @if has_detail {
 div class="border-t bg-surface hidden" {
 div class="border-t bg-surface hidden-grid" {
 div class="border-t bg-surface hidden-item" {
 span class="label" { "BOM 编码:" }
 span class="value" class="font-mono" { "BOM-" (entry.source_id) }
 }
 div class="border-t bg-surface hidden-item" {
 span class="label" { "版本:" }
 span class="value" { (version) }
 }
 @if let Some(pn) = &entry.parent_product_name {
 div class="border-t bg-surface hidden-item" {
 span class="label" { "父件产品:" }
 span class="value" {
 a href=(format!("/admin/md/products/{}", entry.parent_product_code.as_deref().unwrap_or(""))) class="text-accent no-underline" {
 (pn)
 }
 }
 }
 }
 @if let Some(pc) = &entry.parent_product_code {
 div class="border-t bg-surface hidden-item" {
 span class="label" { "父件编码:" }
 span class="value" class="font-mono" { (pc) }
 }
 }
 div class="border-t bg-surface hidden-item" {
 span class="label" { "用量:" }
 span class="value" class="font-mono" {
 (qty) " " (unit)
 }
 }
 @if let Some(remark) = &entry.node_remark {
 @if !remark.is_empty() {
 div class="border-t bg-surface hidden-item" class="col-span-full" {
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
 div class="hidden fixed z-[1100] place-items-center open"
 _="on click remove .open" {
 div class="bg-bg rounded-lg w-[480px]" {
 div class="bg-bg rounded-lg w-[480px]-body" {
 div class="bg-bg rounded-lg w-[480px]-icon-wrap" {
 (icon::circle_alert_icon("w-7 h-7"))
 }
 div class="text-lg font-semibold text-fg text-center mb-2" { "无法删除" }
 p class="text-sm text-muted text-center leading-relaxed" {
 (maud::PreEscaped(format!(
 "产品 <strong>{name}</strong> 正被 <strong>{total}</strong> 个 BOM 引用，无法删除。请先移除相关引用后再试。",
 )))
 }
 }
 div class="bg-bg rounded-lg w-[480px]-foot" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 _="on click remove .open from closest .dialog-overlay" { "知道了" }
 }
 }
 }
 }
}

fn price_history_table(_product_id: i64, entries: &[PriceLogEntry]) -> Markup {
 html! {
 div class="fixed z-[1000] grid place-items-center opacity-0 is-open"
 _="on click remove .is-open" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 { "价格变更记录" }
 button class="text-muted hover:text-fg cursor-pointer bg-transparent border-none"
 _="on click remove .is-open from closest .modal-overlay" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 @if entries.is_empty() {
 div class="text-center p-6 text-muted text-sm" { "暂无价格变更记录" }
 } @else {
 @for entry in entries {
 (price_history_diff_item(entry))
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .is-open from closest .modal-overlay" { "关闭" }
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
 div class="flex items-start gap-[14px] bg-surface rounded-lg" {
 div class="w-[40px] h-[40px] rounded flex items-center justify-center shrink-0" {
 (icon::box_icon("w-5 h-5"))
 }
 div class="flex-1 min-w-0" {
 div class="text-[15px] font-semibold text-fg" { (product.pdt_name) }
 div class="text-[12px] text-muted" {
 (product.product_code) " \u{00b7} "
 @if spec.is_empty() {
 (product.unit)
 } @else {
 (spec) " / " (product.unit)
 }
 }
 }
 }
 // Price section
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::currency_icon("w-3.5 h-3.5"))
 "产品单价"
 }
 div class="flex items-center border border-border rounded overflow-hidden bg-bg" {
 div class="flex items-center border border-border rounded overflow-hidden bg-bg-label" { "单价" }
 div class="prefix" { "¥" }
 input type="text" name="new_price"
 value=(format!("{:.4}", current_price))
 placeholder="0.0000";
 }
 }
 // Remark section
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::comment_icon("w-3.5 h-3.5"))
 "调价说明"
 }
 div class="form-field" {
 textarea name="remark" placeholder="调价原因（如：原材料上涨、供应商调价、季度促销等）" rows="2" class="resize-none w-full text-[13px] text-fg" class="rounded-md" class="border border-border" style="padding:8px 12px;font-family:var(--font-body)" {}
 }
 }
 // Price history
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::clock_icon("w-3.5 h-3.5"))
 "变更历史"
 }
 @if history.is_empty() {
 div class="text-center text-muted text-sm py-8" { "暂无价格变更记录" }
 } @else {
 @for entry in history {
 (price_history_diff_item(entry))
 }
 @if has_more {
 a class="text-center text-[12px] text-accent cursor-pointer" href="/admin/md/price-history" {
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
 div class="border border-border-soft rounded-lg" {
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
 span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap" { (entry.remark) }
 }
 }
 }
 }
}