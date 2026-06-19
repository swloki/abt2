use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::category::model::*;
use abt_core::master_data::category::CategoryService;

use crate::components::icon;
use crate::components::pagination::htmx_pagination_inherited;
use crate::components::modal;
use crate::layout::page::admin_page;
use crate::routes::category::{
 CategoryCreatePath, CategoryDeletePath, CategoryDetailPanelPath, CategoryListPath,
 CategoryUpdatePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub(crate) struct CreateCategoryForm {
 pub category_name: String,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub parent_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateCategoryForm {
 pub category_name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PanelQuery {
 #[serde(default = "default_page")]
 pub page: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
 pub category_id: Option<i64>,
 #[serde(default = "default_page")]
 pub page: u32,
}

fn default_page() -> u32 { 1 }

// ── Handlers ──

#[require_permission("CATEGORY", "read")]
pub async fn get_category_list(
 _path: CategoryListPath,
 ctx: RequestContext,
 Query(query): Query<ListQuery>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("CATEGORY", "create").await;
 let can_update = ctx.has_permission("CATEGORY", "update").await;
 let can_delete = ctx.has_permission("CATEGORY", "delete").await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;
 let svc = state.category_service();
 let tree = svc
 .get_tree(&service_ctx, &mut conn, None, None)
 .await?;

 // Determine which category to show: explicit query param, or first in tree
 let selected_id = query.category_id.or_else(|| tree.first().map(|f| f.category_id));

 let first_panel = if let Some(cat_id) = selected_id {
 let category = svc.get(&service_ctx, &mut conn, cat_id).await.ok();
 if let Some(cat) = category {
 let parent_name = if cat.parent_id != 0 {
 svc.get(&service_ctx, &mut conn, cat.parent_id).await.map(|p| p.category_name.clone()).unwrap_or_else(|_| "—".into())
 } else {
 "—".to_string()
 };
 let update_url = CategoryUpdatePath { id: cat_id }.to_string();
 let delete_url = CategoryDeletePath { id: cat_id }.to_string();
 let mut child_tree = svc.get_tree(&service_ctx, &mut conn, Some(cat_id), None).await.unwrap_or_default();
 if !child_tree.is_empty() {
 let child_ids: Vec<i64> = child_tree.iter().map(|c| c.category_id).collect();
 if let Ok(counts) = svc.count_products_batch(&service_ctx, &mut conn, &child_ids).await {
 for child in &mut child_tree {
 if let Some(cnt) = counts.get(&child.category_id) {
 child.meta.count = *cnt;
 }
 }
 }
 }
 let page = abt_core::shared::types::PageParams::new(query.page, 5);
 let products = svc.list_products(&service_ctx, &mut conn, cat_id, page).await
 .unwrap_or_else(|_| abt_core::shared::types::PaginatedResult::empty(query.page, 5));
 Some((detail_panel(&cat, &parent_name, &update_url, &delete_url, &child_tree, &products, cat_id, can_update, can_delete), cat_id))
 } else {
 None
 }
 } else {
 None
 };

 let content = category_page(&tree, first_panel.as_ref().map(|(p, _)| p), first_panel.as_ref().map(|(_, id)| *id), can_create);
 let page_html = admin_page(
 is_htmx,
 "产品分类",
 &claims,
 "md",
 CategoryListPath::PATH,
 "主数据管理",
 Some("产品分类"),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

#[require_permission("CATEGORY", "read")]
pub async fn get_category_tree(ctx: RequestContext) -> crate::errors::Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.category_service();

 let tree = svc
 .get_tree(&service_ctx, &mut conn, None, None)
 .await?;

 Ok(Html(tree_fragment(&tree, None).into_string()))
}

#[require_permission("CATEGORY", "read")]
pub async fn get_category_detail_panel(
 path: CategoryDetailPanelPath,
 ctx: RequestContext,
 Query(query): Query<PanelQuery>,
) -> crate::errors::Result<Html<String>> {
 let can_update = ctx.has_permission("CATEGORY", "update").await;
 let can_delete = ctx.has_permission("CATEGORY", "delete").await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.category_service();

 let category = svc.get(&service_ctx, &mut conn, path.id).await?;

 let parent_name = if category.parent_id != 0 {
 let parent = svc.get(&service_ctx, &mut conn, category.parent_id).await?;
 parent.category_name.clone()
 } else {
 "—".to_string()
 };

 let update_url = CategoryUpdatePath { id: path.id }.to_string();
 let delete_url = CategoryDeletePath { id: path.id }.to_string();

 // Get child categories for sub-category cards
 let mut child_tree = svc
 .get_tree(&service_ctx, &mut conn, Some(path.id), None)
 .await
 .unwrap_or_default();

 // Enrich sub-category counts with actual product numbers
 if !child_tree.is_empty() {
 let child_ids: Vec<i64> = child_tree.iter().map(|c| c.category_id).collect();
 if let Ok(counts) = svc.count_products_batch(&service_ctx, &mut conn, &child_ids).await {
 for child in &mut child_tree {
 if let Some(cnt) = counts.get(&child.category_id) {
 child.meta.count = *cnt;
 }
 }
 }
 }

 // Get associated products (paginated)
 let page = abt_core::shared::types::PageParams::new(query.page, 5);
 let products = svc
 .list_products(&service_ctx, &mut conn, path.id, page)
 .await
 .unwrap_or_else(|_| abt_core::shared::types::PaginatedResult::empty(query.page, 5));

 Ok(Html(
 detail_panel(&category, &parent_name, &update_url, &delete_url, &child_tree, &products, path.id, can_update, can_delete).into_string(),
 ))
}

#[require_permission("CATEGORY", "create")]
pub async fn create_category(
 _path: CategoryCreatePath,
 ctx: RequestContext,
 Form(form): Form<CreateCategoryForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.category_service();

 let req = CreateCategoryReq {
 category_name: form.category_name,
 parent_id: form.parent_id.unwrap_or(0),
 };

 svc.create(&service_ctx, &mut conn, req).await?;

 Ok(([("HX-Redirect", CategoryListPath::PATH)], Html(String::new())))
}

#[require_permission("CATEGORY", "update")]
pub async fn update_category(
 path: CategoryUpdatePath,
 ctx: RequestContext,
 Form(form): Form<UpdateCategoryForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.category_service();

 let req = UpdateCategoryReq {
 category_name: Some(form.category_name),
 };

 svc.update(&service_ctx, &mut conn, path.id, req).await?;

 Ok(([("HX-Refresh", "true")], Html(String::new())))
}

#[require_permission("CATEGORY", "delete")]
pub async fn delete_category(
 path: CategoryDeletePath,
 ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.category_service();

 svc.delete(&service_ctx, &mut conn, path.id).await?;

 Ok(([("HX-Redirect", CategoryListPath::PATH)], Html(String::new())))
}



// ── Page Component ──

fn category_page(tree: &[CategoryTree], initial_panel: Option<&Markup>, first_id: Option<i64>, can_create: bool) -> Markup {

 html! {
 div {
 script { (category_split_view_script()) }

 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "产品分类" }
 (crate::components::export_button::export_dropdown(&[
 crate::components::export_button::ExportItem {
 label: "导出分类数据".into(),
 export_type: "categories".into(),
 },
 ]))
 }

 // ── Split View Container ──
 div class="flex gap-6 h-[calc(100vh-180px)] min-h-[600px]" {
 div class="w-80 min-w-80 bg-bg border border-border-soft rounded-md shadow-[var(--shadow-xs)] flex flex-col overflow-hidden" {
 div class="p-4 pb-3 border-b border-border-soft shrink-0" {
 h3 class="text-base font-semibold text-fg mb-3" { "分类目录" }
 div class="relative w-full" {
 (icon::search_icon("absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted"))
 input class="w-full pl-8 pr-2 py-1.5 border border-border rounded-sm text-sm bg-surface text-fg outline-none focus:border-accent transition-all duration-150"
 type="text" placeholder="搜索分类…"
 _="on input call filterTree(my value)" {}
 }
 }
 div class="flex-1 overflow-y-auto py-2" id="category-tree" {
 (tree_fragment(tree, first_id))
 }
 @if can_create {
 div class="p-3 px-4 border-t border-border-soft shrink-0" {
 button class="inline-flex items-center justify-center gap-2 w-full py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 _="on click add .is-open to #create-modal" {
 (icon::plus_icon("w-4 h-4"))
 "新建分类"
 }
 }
 }
 }
 div class="flex-1 min-w-0 overflow-y-auto" id="detail-panel" {
 @if let Some(panel) = initial_panel {
 (panel)
 } @else {
 div class="flex flex-col items-center justify-center text-center text-muted min-h-[400px]" {
 svg class="w-16 h-16 text-border mb-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" {
 path d="M4 20h16M8 16h8M6 12h12M10 8h4M12 4v16" {}
 }
 div class="text-base font-medium mb-2" { "请从左侧选择一个分类" }
 div class="text-sm text-muted" { "选择分类查看详情和管理关联产品" }
 }
 }
 }
 }

 // ── Create Modal ──
 (create_category_modal(tree, can_create))
 }
 }
}

// Vanilla JS globals for tree interaction (filterTree).

fn category_split_view_script() -> Markup {
 PreEscaped(r#"<script>
function filterTree(q) {
  q = (q || '').trim().toLowerCase();
  var container = document.querySelector('#category-tree');
  if (!container) return;
  var rows = container.querySelectorAll('[data-name]');
  for (var i = 0; i < rows.length; i++) {
    var name = (rows[i].getAttribute('data-name') || '').toLowerCase();
    rows[i].style.display = (!q || name.indexOf(q) >= 0) ? '' : 'none';
  }
}
</script>"#.to_string())
}

// ── Tree Fragment ──

fn tree_fragment(tree: &[CategoryTree], selected_id: Option<i64>) -> Markup {
 let expand_ids: Vec<i64> = selected_id.map_or(Vec::new(), |sid| {
 fn find_path(nodes: &[CategoryTree], target: i64, path: &mut Vec<i64>) -> bool {
 for n in nodes {
 path.push(n.category_id);
 if n.category_id == target { return true; }
 if find_path(&n.children, target, path) { return true; }
 path.pop();
 }
 false
 }
 let mut p = Vec::new();
 find_path(tree, sid, &mut p);
 p
 });

 html! {
 @for node in tree {
 (tree_node(node, 0, selected_id, &expand_ids))
 }
 }
}

fn tree_node(node: &CategoryTree, depth: usize, selected_id: Option<i64>, expand_ids: &[i64]) -> Markup {
 let has_children = !node.children.is_empty();
 let count = node.meta.count;
 let id = node.category_id;
 let name = &node.category_name;
 let name_lower = name.to_lowercase();
 let detail_url = format!("/admin/md/categories?category_id={}", id);
 let pad = format!("padding-left: {}px", depth * 24 + 16);
 let is_active = selected_id == Some(id);
 let should_expand = is_active || expand_ids.contains(&id);

 let active_cls = "bg-accent-bg before:content-[''] before:absolute before:left-0 before:top-0 before:bottom-0 before:w-[3px] before:bg-accent before:rounded-r-sm";
 let row_cls = if is_active {
 format!("cat-row flex items-center gap-1 px-4 py-1.5 cursor-pointer relative hover:bg-accent-bg transition-colors {}", active_cls)
 } else {
 "cat-row flex items-center gap-1 px-4 py-1.5 cursor-pointer relative hover:bg-accent-bg transition-colors".to_string()
 };
 let name_cls = if is_active { "flex-1 text-sm truncate transition-colors text-accent font-semibold" } else { "flex-1 text-sm truncate transition-colors text-fg" };
 let children_style = if should_expand { "display: block" } else { "display: none" };

 html! {
 @if has_children {
 div class="select-none" data-name=(name_lower) {
 div class=(row_cls)
 style=(pad)
 hx-get=(detail_url)
 hx-select="#detail-panel" hx-target="#detail-panel" hx-swap="innerHTML"
 hx-push-url="true"
 _="on click take .bg-accent-bg from .cat-row then add .bg-accent-bg to me" {
 span class="w-5 h-5 grid place-items-center shrink-0 cursor-pointer rounded-sm hover:bg-black/6"
 _="on click halt the event then toggle .rotate-90 on me then if next <div/>'s style's display is 'none' then show next <div/> else hide next <div/>" {
 (icon::chevron_down_icon(&(format!("w-3.5 h-3.5 text-muted transition-transform{}", if should_expand { " rotate-90" } else { "" }))))
 }
 span class=(name_cls) { (name) }
 @if count > 0 {
 span class="text-[11px] text-muted bg-surface px-2 py-0.5 rounded-full font-medium shrink-0 font-mono tabular-nums" { (count) }
 }
 }
 div class="overflow-hidden" style=(children_style) {
 @for child in &node.children {
 (tree_node(child, depth + 1, selected_id, expand_ids))
 }
 }
 }
 } @else {
 div class="select-none" data-name=(name_lower) {
 div class=(row_cls)
 style=(pad)
 hx-get=(detail_url)
 hx-select="#detail-panel" hx-target="#detail-panel" hx-swap="innerHTML"
 hx-push-url="true"
 _="on click take .bg-accent-bg from .cat-row then add .bg-accent-bg to me" {
 span class="w-5 h-5 shrink-0" {}
 span class=(name_cls) { (name) }
 @if count > 0 {
 span class="text-[11px] text-muted bg-surface px-2 py-0.5 rounded-full font-medium shrink-0 font-mono tabular-nums" { (count) }
 }
 }
 }
 }
 }
}

fn detail_panel(
 category: &Category,
 parent_name: &str,
 update_url: &str,
 delete_url: &str,
 child_tree: &[CategoryTree],
 products: &abt_core::shared::types::PaginatedResult<abt_core::master_data::category::ProductSummary>,
 category_id: i64,
 can_update: bool,
 can_delete: bool,
) -> Markup {
 use abt_core::master_data::product::ProductStatus;

 let has_children = !child_tree.is_empty();
 let has_products = !products.items.is_empty();
 let total_products = products.total;
 let total_pages = products.total_pages;
 let current_page = products.page;

 let subcat_cards: Vec<(String, String, i64)> = child_tree
 .iter()
 .map(|c| (c.category_name.clone(), format!("/admin/md/categories/{}/panel", c.category_id), c.meta.count))
 .collect();

 let info_card = html! {
 div class="data-card" {
 div class="flex items-start justify-between mb-5" {
 div {
 div class="text-xl font-bold text-fg tracking-tight" { (category.category_name) }
 div class="text-sm text-muted font-mono tabular-nums mt-1" {
 "路径: " (category.path) " \u{00a0}·\u{00a0} 上级: " (parent_name)
 }
 }
 div class="flex gap-2" {
 @if can_update {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4"
 _="on click add .is-open to #edit-category-modal" {
 (icon::edit_icon("w-4 h-4"))
 "编辑"
 }
 }
 @if can_delete {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-danger border border-border hover:bg-danger-bg hover:border-[#ffccc7] text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4"
 hx-post=(delete_url)
 hx-confirm="确定要删除此分类吗？此操作不可撤销。"
 hx-swap="none" {
 (icon::trash_icon("w-4 h-4"))
 "删除"
 }
 }
 }
 }
 div class="grid grid-cols-4 gap-5" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "分类名称" }
 span class="text-sm text-fg font-medium" { (category.category_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "分类路径" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (category.path) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "上级分类" }
 span class="text-sm text-fg font-medium" { (parent_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "关联产品数" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (category.meta.count) }
 }
 }
 }
 };

 let subcat_section = html! {
 @if has_children {
 div class="mb-5" {
 div class="flex items-center justify-between mb-4" {
 div {
 span class="text-[13px] font-semibold text-fg" { "子分类" }
 span class="text-xs text-muted ml-2" { "(" (child_tree.len()) ")" }
 }
 }
 div class="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3" {
 @for (name, url, count) in &subcat_cards {
 a class="flex items-center justify-between bg-bg border border-border-soft rounded-md p-4 cursor-pointer transition-all duration-150 shadow-[var(--shadow-xs)] hover:border-accent hover:shadow-[var(--shadow-sm)] hover:-translate-y-px no-underline"
 href=(url)
 hx-get=(url)
 hx-target="#detail-panel" hx-swap="innerHTML" hx-push-url="true" {
 span class="text-sm font-medium text-fg truncate min-w-0" { (name) }
 span class="text-xs text-muted bg-surface px-2.5 py-0.5 rounded-full font-mono tabular-nums" { (count) }
 }
 }
 }
 }
 }
 };

 let products_section = html! {
 div class="mb-5" id="products-section"
 hx-select="#products-section" hx-target="#products-section"
 hx-swap="outerHTML" hx-push-url="true" {
 div class="flex items-center justify-between mb-4" {
 div {
 span class="text-[13px] font-semibold text-fg" { "关联产品" }
 span class="text-xs text-muted ml-2" { "(" (total_products) ")" }
 }
 }
 @if has_products {
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "产品编码" }
 th { "产品名称" }
 th { "状态" }
 }
 }
 tbody {
 @for p in &products.items {
 tr {
 td class="text-accent font-medium font-mono tabular-nums" { (p.product_code) }
 td class="max-w-[260px] truncate" title=(p.pdt_name) { strong { (p.pdt_name) } }
 td {
 @match p.status {
 ProductStatus::Active => span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-success-bg text-success" { "在用" }
 ProductStatus::Inactive => span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted" { "停用" }
 ProductStatus::Obsolete => span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-danger-bg text-danger" { "淘汰" }
 }
 }
 }
 }
 }
 }
 }
 }
 }
 @if total_pages > 1 {
 (htmx_pagination_inherited(
 "/admin/md/categories",
 total_products, current_page, total_pages,
 ))
 }
 @if !has_products {
 div class="bg-surface border border-border-soft rounded-md p-8 text-center text-sm text-muted" {
 "暂无关联产品"
 }
 }
 }
 };

 let edit_modal = modal::modal(
 "edit-category-modal",
 "编辑分类",
 "保存",
 "edit-category-form",
 update_url,
 html! {
 div class="form-field" {
 label { "分类名称" }
 input type="text" name="category_name"
 value=(category.category_name) required;
 }
 },
 );
 html! {
 div {
 (info_card)
 (subcat_section)
 (products_section)
 (edit_modal)
 }
 }
}


// ── Create Category Modal ──

fn create_category_modal(tree: &[CategoryTree], can_create: bool) -> Markup {
 if !can_create {
 return html! {};
 }
 // TODO: Surreal.js migration - modal tied to Alpine categorySplitView component
 modal::modal(
 "create-modal",
 "新建分类",
 "保存分类",
 "create-category-form",
 CategoryCreatePath::PATH,
 html! {
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "分类名称 " span class="text-danger" { "*" } }
 input type="text" name="category_name"
 placeholder="请输入分类名称" required;
 }
 div class="form-field" {
 label { "上级分类" }
 select name="parent_id" {
 option value="0" { "无 (顶级分类)" }
 @for node in tree {
 (tree_option(node, 0))
 }
 }
 }
 }
 },
 )
}

fn tree_option(node: &CategoryTree, depth: usize) -> Markup {
 let prefix = "\u{3000}".repeat(depth);
 html! {
 option value=(node.category_id) { (prefix) (node.category_name) }
 @for child in &node.children {
 (tree_option(child, depth + 1))
 }
 }
}
