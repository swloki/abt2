use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::bom::model::*;
use abt_core::master_data::bom::{
 BomCategoryService, BomCommandService, BomNodeService, BomQueryService,
};
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::PageParams;
use abt_core::shared::types::DomainError;

use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::bom::{
 BomEditPath, BomListPath, BomNodeMovePath, BomNodePath, BomNodesPath, BomProductsPath,
 BomPublishPath, BomSaveAsPath, BomUpdateCategoryPath,
};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
 pub bom_id: Option<i64>,
 pub name: Option<String>,
 pub code: Option<String>,
}

// ── Form requests ──

#[derive(Debug, Deserialize)]
pub struct AddNodeForm {
 pub product_id: i64,
 pub parent_id: i64,
 pub quantity: String,
 #[serde(default)]
 pub unit: Option<String>,
 #[serde(default)]
 pub loss_rate: Option<String>,
 #[serde(default)]
 pub position: Option<String>,
 #[serde(default)]
 pub work_center: Option<String>,
 #[serde(default)]
 pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeForm {
 pub quantity: Option<String>,
 #[serde(default)]
 pub loss_rate: Option<String>,
 #[serde(default)]
 pub unit: Option<String>,
 #[serde(default)]
 pub position: Option<String>,
 #[serde(default)]
 pub work_center: Option<String>,
 #[serde(default)]
 pub remark: Option<String>,
 #[serde(default)]
 pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct SaveAsForm {
 pub new_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryForm {
 pub bom_category_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("BOM", "update")]
pub async fn get_bom_edit(
 path: BomEditPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let bom_svc = state.bom_query_service();
 let product_svc = state.product_service();
 let category_svc = state.bom_category_service();

 let mut bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

 // Resolve product names for all nodes
 let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
 let products = if product_ids.is_empty() {
 Vec::new()
 } else {
 product_svc
 .get_by_ids(&service_ctx, &mut conn, product_ids)
 .await
 .unwrap_or_default()
 };
 let product_map: HashMap<i64, &abt_core::master_data::product::model::Product> =
 products.iter().map(|p| (p.product_id, p)).collect();


 // Filter out nodes whose products no longer exist (and their descendants)
 crate::pages::bom_detail::filter_invalid_nodes(&mut bom.bom_detail.nodes, &product_map);
 // Load BOM categories
 let categories = category_svc
 .list(
 &service_ctx,
 &mut conn,
 BomCategoryQuery::default(),
 PageParams::new(1, 200),
 )
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = bom_edit_page(&bom, &product_map, &categories, claims.sub);
 let edit_path_str = BomEditPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 编辑 BOM", bom.bom_name),
 &claims,
 "md",
 &edit_path_str,
 "主数据管理",
 Some(&bom.bom_name),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: search products → return HTML fragment
#[require_permission("PRODUCT", "read")]
pub async fn get_bom_products(
 ctx: RequestContext,
 Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.product_service();

 let filter = ProductQuery {
 name: params.name.filter(|s| !s.is_empty()),
 code: params.code.filter(|s| !s.is_empty()),
 status: None,
 owner_department_id: None,
 category_id: None,
 };
 let result = svc
 .list(
 &service_ctx,
 &mut conn,
 filter,
 abt_core::shared::types::PageParams::new(1, 20),
 )
 .await?;
 Ok(Html(product_list_fragment(&result.items, params.bom_id).into_string()))
}

/// POST: add a node to BOM
#[require_permission("BOM", "update")]
pub async fn add_node(
 path: BomNodesPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<AddNodeForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let node_svc = state.bom_node_service();
 let quantity: Decimal = form.quantity.parse().unwrap_or(Decimal::ONE);
 let loss_rate: Decimal = form
 .loss_rate
 .as_deref()
 .and_then(|s| s.parse().ok())
 .unwrap_or(Decimal::ZERO);

 // Determine order: max existing order among siblings + 1
 let bom_svc = state.bom_query_service();
 let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;
 let max_order = bom
 .bom_detail
 .nodes
 .iter()
 .filter(|n| n.parent_id == form.parent_id)
 .map(|n| n.order)
 .max()
 .unwrap_or(0);

 node_svc
 .add_node(
 &service_ctx,
 &mut conn,
 path.id,
 NewBomNode {
 product_id: form.product_id,
 quantity,
 parent_id: form.parent_id,
 loss_rate,
 order: max_order + 1,
 unit: form.unit.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 position: form.position.filter(|s| !s.is_empty()),
 work_center: form.work_center.filter(|s| !s.is_empty()),
 properties: None,
 },
 )
 .await?;

 let redirect = BomEditPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: update a node
#[require_permission("BOM", "update")]
pub async fn update_node(
 path: BomNodePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<UpdateNodeForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let node_svc = state.bom_node_service();

 let quantity: Option<Decimal> = form
 .quantity
 .as_deref()
 .and_then(|s| s.parse().ok());
 let loss_rate: Option<Decimal> = form
 .loss_rate
 .as_deref()
 .and_then(|s| s.parse().ok());

 node_svc
 .update_node(
 &service_ctx,
 &mut conn,
 path.id,
 path.node_id,
 UpdateBomNodeReq {
 quantity,
 loss_rate,
 order: None,
 unit: form.unit.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 position: form.position.filter(|s| !s.is_empty()),
 work_center: form.work_center.filter(|s| !s.is_empty()),
 properties: None,
 },
 form.expected_version,
 )
 .await?;

 Ok(([("HX-Trigger", "nodeUpdated")], Html(String::new())))
}

/// DELETE: delete a node
#[require_permission("BOM", "update")]
pub async fn delete_node(
 path: BomNodePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let node_svc = state.bom_node_service();
 node_svc
 .delete_node(&service_ctx, &mut conn, path.id, path.node_id)
 .await?;

 Ok(([("HX-Trigger", "nodeUpdated")], Html(String::new())))
}

/// POST: move a node (drag-and-drop reorder)
#[derive(Debug, Deserialize)]
pub struct MoveNodeForm {
 pub new_parent_id: i64,
 #[serde(default, deserialize_with = "deserialize_optional_i64")]
 pub before_sibling_id: Option<i64>,
}

fn deserialize_optional_i64<'de, D>(de: D) -> std::result::Result<Option<i64>, D::Error>
where
 D: serde::Deserializer<'de>,
{
 let opt: Option<String> = Option::deserialize(de)?;
 match opt {
 None => Ok(None),
 Some(ref s) if s.is_empty() => Ok(None),
 Some(s) => s.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
 }
}

#[require_permission("BOM", "update")]
pub async fn move_node(
 path: BomNodeMovePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<MoveNodeForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let node_svc = state.bom_node_service();
 node_svc
 .move_node(
 &service_ctx,
 &mut conn,
 path.id,
 path.node_id,
 form.new_parent_id,
 form.before_sibling_id,
 )
 .await?;

 let redirect = BomEditPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: update BOM category
#[require_permission("BOM", "update")]
pub async fn update_category(
 path: BomUpdateCategoryPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<UpdateCategoryForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let cmd_svc = state.bom_command_service();
 cmd_svc
 .update(
 &service_ctx,
 &mut conn,
 path.id,
 UpdateBomReq {
 name: None,
 bom_category_id: form.bom_category_id,
 },
 0,
 )
 .await?;

 let redirect = BomEditPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: save BOM as new copy
#[require_permission("BOM", "create")]
pub async fn save_as(
 path: BomSaveAsPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<SaveAsForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let cmd_svc = state.bom_command_service();
 let new_id = cmd_svc
 .save_as(&service_ctx, &mut conn, path.id, form.new_name)
 .await?;

 let redirect = BomEditPath { id: new_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// HTMX: return edit form HTML fragment for a node
#[require_permission("BOM", "update")]
pub async fn get_node_edit_form(
 path: BomNodePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let bom_svc = state.bom_query_service();
 let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;
 let node = bom.bom_detail.nodes.iter().find(|n| n.id == path.node_id)
 .ok_or_else(|| DomainError::not_found("节点不存在"))?;
 Ok(Html(node_edit_form_fragment(path.id, path.node_id, bom.version, node).into_string()))
}


fn node_edit_form_fragment(bom_id: i64, node_id: i64, bom_version: i32, node: &BomNode) -> Markup {
 let action = BomNodePath { id: bom_id, node_id }.to_string();
 html! {
 div class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0" onclick="event.stopPropagation()" {
 div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
 h2 { "编辑节点" }
 button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
 _="on click remove .is-open from #bom-edit-modal then empty #bom-edit-modal" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 form hx-post=(action) hx-swap="none" {
 input type="hidden" name="expected_version" value=(bom_version) {}
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "数量 " span style="color:var(--danger)" { "*" } }
 input type="number" name="quantity" step="0.01" min="0.01" required value=(node.quantity) {}
 }
 div class="form-field" {
 label { "损耗率%" }
 input type="number" name="loss_rate" step="0.1" min="0" value=(node.loss_rate) {}
 }
 div class="form-field" {
 label { "单位" }
 input type="text" name="unit" value=(node.unit.as_deref().unwrap_or("")) {}
 }
 div class="form-field" {
 label { "工作中心" }
 input type="text" name="work_center" value=(node.work_center.as_deref().unwrap_or("")) {}
 }
 div class="form-field" {
 label { "位置" }
 input type="text" name="position" value=(node.position.as_deref().unwrap_or("")) {}
 }
 div class="form-field field-full" {
 label { "备注" }
 input type="text" name="remark" value=(node.remark.as_deref().unwrap_or("")) {}
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" style="padding:var(--space-4) 0 0;border-top:1px solid var(--border-soft)" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .is-open from #bom-edit-modal then empty #bom-edit-modal" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "保存" }
 }
 }
 }
 }
 }
}

// ── Components ──

fn bom_edit_page(
 bom: &Bom,
 product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
 categories: &[BomCategory],
 current_user_id: i64,
) -> Markup {
 let list_path = BomListPath;
 let publish_path = BomPublishPath { id: bom.bom_id };
 let node_count = bom.bom_detail.nodes.len();

 let (status_label, status_class) = bom_status_display(bom.status);
 let is_draft = bom.status == BomStatus::Draft;
 let is_owner = bom.created_by.map(|id| id == current_user_id).unwrap_or(false);

 // Build depth map and parent set
 let depth_map = build_depth_map(&bom.bom_detail.nodes);
 let parent_ids: HashSet<i64> = bom
 .bom_detail
 .nodes
 .iter()
 .filter(|n| n.parent_id != 0)
 .map(|n| n.parent_id)
 .collect();
 // Build ancestors map for collapse: each node → ordered list of ancestor node IDs
 let ancestors_map = build_ancestors_map(&bom.bom_detail.nodes);

 // Max level for filter
 let max_level = depth_map.values().copied().max().map(|d| d + 1).unwrap_or(0);
 html! {
 div id="bom-edit-app" hx-get=(BomEditPath { id: bom.bom_id }.to_string()) hx-trigger="nodeUpdated from:body" hx-select="#bom-edit-app" hx-swap="outerHTML" hx-disinherit="hx-select" {
 // ── Toolbar ──
 div class="flex flex-wrap items-center justify-between gap-3" {
 // Left side: back, category, view toggle, level filter
 div class="flex flex-wrap items-center justify-between gap-3-left" {
 a class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs [&_svg]:w-4 [&_svg]:h-4" href=(format!("{list_path}?restore=true")) {
 (icon::arrow_left_icon("w-4 h-4"))
 " 返回列表"
 }

 // Category selector
 @if !categories.is_empty() {
 div class="relative" {
 select name="bom_category_id"
 hx-post=(BomUpdateCategoryPath { id: bom.bom_id }.to_string())
 hx-trigger="change"
 hx-swap="none"
 hx-confirm="确定要更改分类吗？" {
 option value="" selected[bom.bom_category_id.is_none()] { "未分类" }
 @for cat in categories {
 option value=(cat.bom_category_id)
 selected[bom.bom_category_id == Some(cat.bom_category_id)] {
 (cat.bom_category_name)
 }
 }
 }
 }
 }

 // Level filter
 select id="bom-level-filter" class="h-[32px] text-sm font-medium bg-white border border-border text-fg-2 rounded-sm cursor-pointer" {
 option value="0" { "全部层级" }
 @for lv in 1..=max_level {
 option value=(lv) { "层级 " (lv) }
 }
 }

 button type="button" class="h-[32px] text-sm font-medium bg-white border border-border text-fg-2 rounded-sm cursor-pointer" id="bom-collapse-all-btn"
 onclick="bomToggleAllCollapse()" {
 "全部折叠"
 }
 }

 // Right side: publish/unpublish, add/save-as, labor cost
 div class="flex flex-wrap items-center justify-between gap-3-right" {
 @if !is_draft && is_owner {
 button class="btn btn-sm inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-ghost [&_svg]:w-4 [&_svg]:h-4" id="bom-publish-btn"
 _="on click add .open to #bom-publish-dialog" {
 (icon::return_arrow_icon("w-4 h-4"))
 " 取消发布"
 }
 } @else if is_draft {
 button class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-[#10b981] text-[#fff] [&_svg]:w-4 [&_svg]:h-4" id="bom-publish-btn"
 _="on click add .open to #bom-publish-dialog"
 disabled[node_count == 0]
 title="请先添加物料" {
 (icon::rocket_icon("w-4 h-4"))
 " 发布"
 }
 }

 @if node_count == 0 {
 button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_svg]:w-4 [&_svg]:h-4" id="bom-add-root-btn"
 _="on click put '0' into <input[name='parent_id']/>'s value then add .is-open to #bom-add-modal then call bomLoadProducts()" {
 (icon::plus_icon("w-4 h-4"))
 " 添加根节点"
 }
 } @else {
 button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-[#10b981] text-[#fff] [&_svg]:w-4 [&_svg]:h-4" id="bom-save-as-btn"
 data-name=(bom.bom_name)
 _="on click put (my @data-name + '_副本') into <input[name='new_name']/>'s value then add .is-open to #bom-save-as-modal" {
 (icon::copy_icon("w-4 h-4"))
 " 另存为"
 }
 }

 a class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-[#f97316] text-[#fff] [&_svg]:w-4 [&_svg]:h-4" href=(format!("/admin/labor/bom-cost/{}", bom.bom_id)) {
 (icon::currency_icon("w-4 h-4"))
 " 人工成本"
 }
 }
 }

 // ── Title ──
 h1 class="text-xl font-bold text-fg tracking-tight" style="display:flex;align-items:center;gap:var(--space-2);margin-bottom:var(--space-4)" {
 (bom.bom_name)
 span class=(format!("status-pill {status_class}")) { (status_label) }
 }

 // ── Node Table ──
 div class="data-card" style="padding:0;overflow:hidden" {
 @if bom.bom_detail.nodes.is_empty() {
 div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
 "暂无组件数据，请点击上方按钮添加根节点"
 }
 } @else {
 div style="overflow-x:auto" {
 table class="w-full text-[13px]" style="table-layout:fixed;min-width:900px" {
 thead {
 tr {
 th style="width:32px" { }
 th style="width:40px" { "编号" }
 th style="width:40px" { "层级" }
 th style="width:120px" { "产品编码" }
 th class="bom-col-name" { "产品" }
 th style="width:100px" { "工作中心" }
 th style="width:80px" { "数量" }
 th style="width:60px" { "单位" }
 th style="width:50px" { "损耗率" }
 th style="width:100px" { "位置" }
 th style="width:90px" { "备注" }
 th style="width:120px" { "操作" }
 }
 }
 tbody id="bom-sortable-tbody" {
 @for (idx, node) in bom.bom_detail.nodes.iter().enumerate() {
 @let depth = *depth_map.get(&node.id).unwrap_or(&0);
 @let level = depth + 1;
 @let has_children = parent_ids.contains(&node.id);
 @let product = product_map.get(&node.product_id);
 @let ancestors = ancestors_map.get(&node.id).map(|v| v.as_slice()).unwrap_or(&[]);
 (bom_node_row(bom.bom_id, idx, level, has_children, node, product.map(|v| &**v), ancestors))
 }
 }
 }
 }
 }
 }

 // ── Add Node Modal ──
 div id="bom-add-modal" class="fixed z-[1000] grid place-items-center opacity-0"
 _="on click[me is event.target] remove .is-open" {
 div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0" onclick="event.stopPropagation()" {
 div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
 h2 { "添加物料" }
 button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
 _="on click remove .is-open from #bom-add-modal" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" hx-disinherit="hx-select" {
 input type="hidden" name="parent_id" value="0" {}
 div class="flex gap-4 p-4 border-b" {
 input type="hidden" name="bom_id" value=(bom.bom_id) {}
 div class="flex-1 flex flex-col gap-[4px]" {
 label class="text-[12px] font-medium text-fg-2" { "产品名称" }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name" placeholder="输入产品名称…"
 hx-get=(BomProductsPath::PATH)
 hx-trigger="keyup changed delay:300ms"
 hx-sync="this:replace"
 hx-target="#bom-edit-product-results"
 hx-swap="innerHTML"
 hx-include=".product-search-bar" {}
 }
 div class="flex-1 flex flex-col gap-[4px]" {
 label class="text-[12px] font-medium text-fg-2" { "产品编码" }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
 hx-get=(BomProductsPath::PATH)
 hx-trigger="keyup changed delay:300ms"
 hx-sync="this:replace"
 hx-target="#bom-edit-product-results"
 hx-swap="innerHTML"
 hx-include=".product-search-bar" {}
 }
 button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
 hx-get=(BomProductsPath::PATH)
 hx-target="#bom-edit-product-results"
 hx-swap="innerHTML"
 _="on click set <.product-search-input/>'s value to '' then trigger keyup on the first <.product-search-input/>" {
 "清除"
 }
 }
 div id="bom-edit-product-results" style="min-height:200px;max-height:320px;overflow-y:auto" {
 div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
 "搜索产品或直接输入关键词…"
 }
 }
 }
 }
 }

 // ── Edit Node Modal (content loaded via HTMX) ──
 div id="bom-edit-modal" class="fixed z-[1000] grid place-items-center opacity-0" _="on htmx:afterSettle if detail.xhr.responseText !== '' add .is-open on click[me is event.target] remove .is-open" { }

 // ── Delete Confirm ──
 (crate::components::confirm_dialog::confirm_dialog(
 "bom-delete-dialog",
 "确认删除",
 "确定要删除该节点及其所有子节点吗？此操作不可撤销。",
 "确认删除",
 "bom-node-delete-form",
 html! {
 form id="bom-node-delete-form" style="display:none"
 hx-swap="none" {}
 },
 ))

 // ── Publish / Unpublish Confirm Dialog ──
 @if !is_draft && is_owner {
 (crate::components::confirm_dialog::confirm_dialog(
 "bom-publish-dialog",
 "确认取消发布",
 "确定要取消发布此 BOM 吗？取消后可重新编辑。",
 "确认取消发布",
 "publish-bom-form",
 html! {
 form id="publish-bom-form" class="hidden"
 hx-post=(publish_path.to_string())
 hx-swap="none" {}
 },
 ))
 } @else if is_draft {
 (crate::components::confirm_dialog::confirm_dialog(
 "bom-publish-dialog",
 "确认发布",
 "确定要发布此 BOM 吗？发布后将无法修改。",
 "确认发布",
 "publish-bom-form",
 html! {
 form id="publish-bom-form" class="hidden"
 hx-post=(publish_path.to_string())
 hx-swap="none" {}
 },
 ))
 }


 // ── Save As Modal ──
 div id="bom-save-as-modal" class="fixed z-[1000] grid place-items-center opacity-0"
 _="on click[me is event.target] remove .is-open" {
 div class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0" onclick="event.stopPropagation()" {
 div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
 h2 { "另存为" }
 button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
 _="on click remove .is-open from #bom-save-as-modal" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 form hx-post=(BomSaveAsPath { id: bom.bom_id }.to_string())
 hx-swap="none" {
 div class="form-field" {
 label { "新 BOM 名称 " span style="color:var(--danger)" { "*" } }
 input type="text" name="new_name" required
 placeholder="输入新的 BOM 名称" {}
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" style="padding:var(--space-4) 0 0;border-top:1px solid var(--border-soft)" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click remove .is-open from #bom-save-as-modal" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-[#10b981] text-[#fff]" { "确认另存为" }
 }
 }
 }
 }
 }

 // ── BOM edit page JS ──
 script src="/bom-edit.js?v=20260604" {}
 }
 }
}

fn bom_node_row(
 bom_id: i64,
 index: usize,
 level: usize,
 has_children: bool,
 node: &BomNode,
 product: Option<&abt_core::master_data::product::model::Product>,
 ancestors: &[i64],
) -> Markup {
 let code = node.product_code.as_deref().or_else(|| product.map(|p| p.product_code.as_str())).unwrap_or("—");
 let name = product.map(|p| p.pdt_name.as_str()).unwrap_or("—");
 let unit = node.unit.as_deref().unwrap_or("—");
 let position = node.position.as_deref().filter(|s| !s.is_empty()).unwrap_or("—");
 let work_center = node
 .work_center
 .as_deref()
 .filter(|s| !s.is_empty())
 .unwrap_or("—");
 let remark = node.remark.as_deref().filter(|s| !s.is_empty()).unwrap_or("");
 let loss_rate = if node.loss_rate == Decimal::ZERO {
 "—".to_string()
 } else {
 format!("{}%", node.loss_rate)
 };
 let row_class = if level == 1 {
 "bom-row-level-0"
 } else if has_children {
 "bom-row-level-1"
 } else {
 "bom-row-level-default"
 };
 let ancestors_str = ancestors.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
 let _indent_px = (level - 1) * 24;
 html! {
 tr class=(row_class) draggable="true"
 data-node-id=(node.id) data-parent-id=(node.parent_id) data-level=(level) data-ancestors=(ancestors_str) {
 td style="text-align:center" {
 @if has_children {
 button type="button" class="inline-flex items-center justify-center w-[20px] h-[20px] border-none rounded-sm cursor-pointer shrink-0"
 onclick=(format!("bomToggleCollapse({})", node.id)) {
 (icon::chevron_down_icon("bom-collapse-icon"))
 }
 }
 }
 td style="text-align:center" { (index + 1) }
 td style="text-align:center" { (level) }
 td class="font-mono tabular-nums" { (code) }
 td class="bom-col-name" { (name) }
 td { (work_center) }
 td class="font-mono tabular-nums" style="text-align:right" { (node.quantity) }
 td { (unit) }
 td style="text-align:right" { (loss_rate) }
 td { (position) }
 td style="color:var(--muted)" { (remark) }
 td {
 div style="display:flex;gap:var(--space-1)" {
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="添加子节点"
 _=(format!("on click put '{}' into <input[name='parent_id']/>'s value then add .is-open to #bom-add-modal then call bomLoadProducts()", node.id)) {
 (icon::plus_icon("w-3.5 h-3.5"))
 }
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑"
 hx-get=(format!("/admin/md/boms/{}/nodes/{}", bom_id, node.id))
 hx-target="#bom-edit-modal" hx-swap="innerHTML" {
 (icon::edit_icon("w-3.5 h-3.5"))
 }
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
 _=(format!("on click set #bom-node-delete-form's @hx-delete to '/admin/md/boms/{}/nodes/{}' then call htmx.process(document.querySelector('#bom-node-delete-form')) then add .open to #bom-delete-dialog", bom_id, node.id)) {
 (icon::trash_icon("w-3.5 h-3.5"))
 }
 }
 }
 }
 }
}


// ── Helpers ──

fn bom_status_display(status: BomStatus) -> (&'static str, &'static str) {
 match status {
 BomStatus::Draft => ("草稿", "status-bom-draft"),
 BomStatus::Published => ("已发布", "status-bom-published"),
 }
}

fn build_depth_map(nodes: &[BomNode]) -> HashMap<i64, usize> {
 let mut depth_map: HashMap<i64, usize> = HashMap::with_capacity(nodes.len());
 for node in nodes {
 let depth = if node.parent_id == 0 {
 0
 } else {
 depth_map.get(&node.parent_id).copied().unwrap_or(0) + 1
 };
 depth_map.insert(node.id, depth);
 }
 depth_map
}

fn build_ancestors_map(nodes: &[BomNode]) -> HashMap<i64, Vec<i64>> {
 let mut ancestors_map: HashMap<i64, Vec<i64>> = HashMap::with_capacity(nodes.len());
 for node in nodes {
 if node.parent_id == 0 {
 ancestors_map.insert(node.id, Vec::new());
 } else if let Some(parent_ancestors) = ancestors_map.get(&node.parent_id).cloned() {
 let mut ancestors = parent_ancestors;
 ancestors.push(node.parent_id);
 ancestors_map.insert(node.id, ancestors);
 } else {
 ancestors_map.insert(node.id, vec![node.parent_id]);
 }
 }
 ancestors_map
}

/// Product search results fragment
fn product_list_fragment(products: &[abt_core::master_data::product::model::Product], bom_id: Option<i64>) -> Markup {
 let bid = bom_id.unwrap_or(0);
 html! {
 @if products.is_empty() {
 div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
 (icon::package_icon("w-8 h-8"))
 p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的产品" }
 }
 } @else {
 div class="py-2" {
 @for p in products {
 div class="flex items-center justify-between p-3 border-b" {
 div class="product-select-info" {
 div class="text-sm font-medium text-fg" { (p.pdt_name) }
 div class="text-[12px] text-muted flex items-center gap-[6px] flex-wrap" {
 span class="bg-surface rounded-sm" { (p.product_code) }
 span class="text-border" { "·" }
 span { (p.meta.specification) }
 span class="text-border" { "·" }
 span { (p.unit) }
 }
 }
 form hx-post=(format!("/admin/md/boms/{}/nodes", bid))
 hx-swap="none"
 hx-include="[name='parent_id']" {
 input type="hidden" name="product_id" value=(p.product_id) {}
 input type="hidden" name="quantity" value="1" {}
 input type="hidden" name="unit" value=(p.unit) {}
 button type="submit" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_svg]:w-4 [&_svg]:h-4" { "选择" }
 }
 }
 }
 }
 }
 }
}
