use std::collections::HashMap;

use axum::response::Html;
use maud::{Markup, html};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::bom::{BomCommandService, BomCostService, BomQueryService};
use abt_core::master_data::bom::model::*;
use abt_core::master_data::product::ProductService;

use abt_macros::require_permission;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::bom::{BomCostDrawerPath, BomCostTempPricePath, BomCostClearTempPath, BomDeletePath, BomDetailPath, BomEditPath, BomLaborCostDrawerPath, BomListPath, BomPublishPath};
use crate::utils::RequestContext;

#[derive(Deserialize)]
pub struct BomDetailQuery {
 pub action: Option<String>,
}
pub async fn get_bom_detail(
 path: BomDetailPath,
 axum::extract::Query(query): axum::extract::Query<BomDetailQuery>,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let can_view_cost = ctx.has_permission("COST", "read").await;
 let can_view_labor_cost = ctx.has_permission("LABOR_COST", "read").await;
 let can_edit = ctx.has_permission("BOM", "update").await;
 let can_delete = ctx.has_permission("BOM", "delete").await;
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let bom_svc = state.bom_query_service();
 let product_svc = state.product_service();

 let mut bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

 // Resolve product names & specs for all nodes
 let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
 let products = if product_ids.is_empty() {
 Vec::new()
 } else {
 product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
 };
 let product_map: HashMap<i64, &abt_core::master_data::product::model::Product> =
 products.iter().map(|p| (p.product_id, p)).collect();

 // Filter out nodes whose products no longer exist (and their descendants)
 filter_invalid_nodes(&mut bom.bom_detail.nodes, &product_map);
 let content = bom_detail_page(&bom, &product_map, can_view_cost, can_view_labor_cost, can_edit, can_delete, query.action.as_deref());
 let detail_path_str = BomDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - BOM 详情", bom.bom_name),
 &claims,
 "md",
 &detail_path_str,
 "主数据管理",
 Some(&bom.bom_name),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}


#[require_permission("BOM", "update")]
pub async fn publish_bom(
 path: BomPublishPath,
 ctx: RequestContext,
) -> crate::errors::Result<impl axum::response::IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let query_svc = state.bom_query_service();
 let bom = query_svc.get(&service_ctx, &mut conn, path.id).await?;

 let cmd_svc = state.bom_command_service();
 if bom.status == BomStatus::Published {
 cmd_svc.unpublish(&service_ctx, &mut conn, path.id).await?;
 } else {
 cmd_svc.publish(&service_ctx, &mut conn, path.id).await?;
 }

 let redirect = BomDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}
// ── Temp Price Session Helpers ──

fn temp_prices_session_key(bom_id: i64) -> String {
 format!("bom_temp_prices:{}", bom_id)
}

async fn load_temp_prices(session: &tower_sessions::Session, bom_id: i64) -> HashMap<i64, String> {
 session.get::<HashMap<i64, String>>(&temp_prices_session_key(bom_id))
 .await
 .ok()
 .flatten()
 .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
pub struct TempPriceForm {
 pub product_id: i64,
 pub temp_price: String,
}

#[require_permission("COST", "read")]
pub async fn get_cost_drawer(
 path: BomCostDrawerPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, session, .. } = ctx;

 let cost_svc = state.bom_cost_service();
 let report = cost_svc.get_cost_report(&service_ctx, &mut conn, path.id, None).await?;
 let temp_prices = load_temp_prices(&session, path.id).await;

 Ok(Html(cost_drawer_content(&report, &temp_prices).into_string()))
}

#[require_permission("COST", "read")]
pub async fn save_temp_price(
 path: BomCostTempPricePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<TempPriceForm>,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, session, .. } = ctx;

 let price = form.temp_price.trim().parse::<Decimal>().unwrap_or(Decimal::ZERO);
 let mut temp_prices = load_temp_prices(&session, path.id).await;
 if price > Decimal::ZERO {
 temp_prices.insert(form.product_id, form.temp_price.trim().to_string());
 } else {
 temp_prices.remove(&form.product_id);
 }
 let _ = session.insert(&temp_prices_session_key(path.id), &temp_prices).await;

 let cost_svc = state.bom_cost_service();
 let report = cost_svc.get_cost_report(&service_ctx, &mut conn, path.id, None).await?;
 let temp_prices = load_temp_prices(&session, path.id).await;

 Ok(Html(cost_drawer_content(&report, &temp_prices).into_string()))
}

#[require_permission("COST", "read")]
pub async fn clear_temp_prices(
 path: BomCostClearTempPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, session, .. } = ctx;

 let _ = session.remove::<HashMap<i64, String>>(&temp_prices_session_key(path.id)).await;

 let cost_svc = state.bom_cost_service();
 let report = cost_svc.get_cost_report(&service_ctx, &mut conn, path.id, None).await?;
 let temp_prices = load_temp_prices(&session, path.id).await;

 Ok(Html(cost_drawer_content(&report, &temp_prices).into_string()))
}

#[require_permission("LABOR_COST", "read")]
pub async fn get_labor_cost_drawer(
 path: BomLaborCostDrawerPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let cost_svc = state.bom_cost_service();
 let report = cost_svc.get_labor_cost_report(&service_ctx, &mut conn, path.id).await?;
 let bom_svc = state.bom_query_service();
 let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

 Ok(Html(labor_cost_drawer_content(&bom.bom_name, &report).into_string()))
}

// ── Components ──

fn bom_detail_page(
 bom: &Bom,
 product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
 can_view_cost: bool,
 can_view_labor_cost: bool,
 can_edit: bool,
 can_delete: bool,
 auto_open: Option<&str>,
) -> Markup {
 let list_path = BomListPath;
 let delete_path = BomDeletePath { id: bom.bom_id };
 let publish_path = BomPublishPath { id: bom.bom_id };
 let cost_drawer_path = BomCostDrawerPath { id: bom.bom_id };
 let labor_drawer_path = BomLaborCostDrawerPath { id: bom.bom_id };
 let node_count = bom.bom_detail.nodes.len();
 let depth_map = build_depth_map(&bom.bom_detail.nodes);

 // Build set of parent IDs to know which nodes have children
 let parent_ids: std::collections::HashSet<i64> = bom.bom_detail.nodes.iter()
 .filter(|n| n.parent_id != 0)
 .map(|n| n.parent_id)
 .collect();

 let (status_label, status_class) = bom_status_display(bom.status);
 let is_draft = bom.status == BomStatus::Draft;

 html! {
 div {
 // ── Detail Top ──
 div class="flex justify-between items-start" {
 div class="flex items-center gap-5" {
 div class="inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none bg-[#e0e7ff]" {
 (icon::clipboard_list_icon("w-5 h-5"))
 }
 div {
 h1 class="text-xl font-bold" {
 (bom.bom_name)
 " "
 span class="bg-accent-bg text-accent rounded-full text-[11px] font-medium" { "v" (bom.version) }
 " "
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 div class="flex gap-4 text-muted text-xs" {
 span { "节点: " (node_count) }
 @if let Some(cat_id) = bom.bom_category_id {
 span { "分类ID: " (cat_id) }
 }
 span { "创建: " (bom.create_at.format("%Y-%m-%d")) }
 }
 }
 }
 div class="flex gap-2 flex-wrap" {
 a class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent cursor-pointer transition-all duration-150 no-underline" href=(format!("{list_path}?restore=true")) {
 (icon::arrow_left_icon("w-3.5 h-3.5"))
 "返回列表"
 }
 @if can_view_cost {
 button type="button" class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent cursor-pointer transition-all duration-150"
 hx-get=(cost_drawer_path.to_string())
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML"
 {
 (icon::currency_icon("w-3.5 h-3.5"))
 "查看成本"
 }
 } @else if can_view_labor_cost {
 button type="button" class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent cursor-pointer transition-all duration-150"
 hx-get=(labor_drawer_path.to_string())
 hx-target="#labor-drawer-body"
 hx-swap="innerHTML"
 _="on click show #labor-drawer" {
 (icon::bolt_icon("w-3.5 h-3.5"))
 "查看人工成本"
 }
 }
 @if can_edit {
 a class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-accent text-white border-none hover:bg-accent-hover cursor-pointer transition-all duration-150 no-underline" href=(BomEditPath { id: bom.bom_id }) {
 (icon::edit_icon("w-3.5 h-3.5"))
 "编辑"
 }
 }
 @if can_edit && is_draft {
 button type="button" class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-accent text-white border-none hover:bg-accent-hover cursor-pointer transition-all duration-150"
 hx-confirm="确定要发布此 BOM 吗？发布后将无法修改。"
 hx-post=(publish_path.to_string())
 hx-swap="none" {
 (icon::check_circle_icon("w-3.5 h-3.5"))
 "发布"
 }
 }
 @if can_delete {
 button type="button" class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-danger text-white border-none hover:opacity-90 cursor-pointer transition-all duration-150"
 hx-confirm=(format!("确定要删除 BOM {} 吗？此操作不可撤销。", bom.bom_name))
 hx-post=(delete_path.to_string())
 hx-target="body"
 hx-swap="outerHTML" {
 (icon::trash_icon("w-3.5 h-3.5"))
 "删除"
 }
 }
 button type="button" class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent cursor-pointer transition-all duration-150"
 hx-post=(format!("{}/bom?bom_id={}", crate::routes::excel::EXPORT_START_PATH, bom.bom_id))
 hx-confirm="确定要导出 BOM 吗？"
 hx-swap="none" {
 (icon::download_icon("w-3.5 h-3.5"))
 "导出"
 }
 }
 }

 // ── BOM结构 ──
 div class="mt-6 bg-white border border-border-soft rounded-md overflow-hidden" {
 div class="flex items-center justify-between px-5 pt-5 pb-3" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg" {
 (icon::clipboard_list_icon("w-4 h-4 text-muted"))
 span { "BOM结构" }
 }
 span class="text-muted font-normal text-xs" {
 "（共 " (node_count) " 个节点）"
 }
 }
 @if bom.bom_detail.nodes.is_empty() {
 div class="text-center py-8 text-muted text-sm" { "暂无BOM节点" }
 } @else {
 div class="overflow-x-auto" {
 table class="w-full border-collapse" {
 thead {
 tr {
 th class="w-[40px] px-3 py-3 text-center text-xs font-semibold text-white bg-accent whitespace-nowrap" { "编号" }
 th class="w-[40px] px-3 py-3 text-center text-xs font-semibold text-white bg-accent whitespace-nowrap" { "层级" }
 th class="w-[120px] px-3 py-3 text-left text-xs font-semibold text-white bg-accent whitespace-nowrap" { "产品编码" }
 th class="w-[200px] px-3 py-3 text-left text-xs font-semibold text-white bg-accent whitespace-nowrap" { "产品" }
 th class="w-[80px] px-3 py-3 text-left text-xs font-semibold text-white bg-accent whitespace-nowrap" { "工作中心" }
 th class="w-[80px] px-3 py-3 text-right text-xs font-semibold text-white bg-accent whitespace-nowrap" { "数量" }
 th class="w-[60px] px-3 py-3 text-center text-xs font-semibold text-white bg-accent whitespace-nowrap" { "单位" }
 th class="w-[50px] px-3 py-3 text-right text-xs font-semibold text-white bg-accent whitespace-nowrap" { "损耗率" }
 th class="w-[150px] px-3 py-3 text-left text-xs font-semibold text-white bg-accent whitespace-nowrap" { "备注" }
 }
 }
 tbody {
 @for (idx, node) in bom.bom_detail.nodes.iter().enumerate() {
 @let depth = *depth_map.get(&node.id).unwrap_or(&0);
 @let level = depth + 1;
 @let has_children = parent_ids.contains(&node.id);
 @let product = product_map.get(&node.product_id);
 (bom_node_row(idx, level, has_children, node, product.map(|v| &**v)))
 }
 }
 }
 }
 }
}



 @if can_view_cost {
 // ── Cost Drawer (wider: 1000px) ──
 @let cost_overlay_cls = if auto_open == Some("cost") { "drawer-overlay open fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]" } else { "drawer-overlay fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]" };
 div id="cost-drawer" class=(cost_overlay_cls)
 _="on click[me is event.target] remove .open from me" {
 div id="costpanel" class="drawer-panel bg-white h-full w-[1000px] max-w-[100vw] flex flex-col shadow-[-8px_0_30px_rgba(0,0,0,0.1)]"
 _="on click halt the event on htmx:afterSettle add .open to #cost-drawer" {
 div class="flex items-center justify-between px-6 py-4 border-b border-border-soft sticky top-0 bg-white z-10" {
 h2 class="flex items-center gap-2 text-base font-semibold text-fg m-0" {
 (icon::currency_icon("w-5 h-5 text-muted"))
 " BOM成本报告"
 }
 button type="button" class="w-8 h-8 border-none bg-transparent cursor-pointer text-muted rounded-md grid place-items-center hover:bg-surface hover:text-fg transition-colors"
 _="on click remove .open from closest .drawer-overlay" {
 "×"
 }
 }
 @if auto_open == Some("cost") {
 div class="flex-1 overflow-y-auto p-6" {
 div id="cost-drawer-body"
 hx-get=(cost_drawer_path.to_string())
 hx-trigger="load"
 hx-swap="innerHTML" {
 div class="text-center text-muted py-10" { "加载中..." }
 }
 }
 } @else {
 div class="flex-1 overflow-y-auto p-6" {
 div id="cost-drawer-body" {}
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from closest .drawer-overlay" { "关闭" }
 }
 }
 }
 }
 @if can_view_labor_cost {
 @let labor_overlay_cls = if auto_open == Some("labor") { "drawer-overlay open fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]" } else { "drawer-overlay fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]" };
 div id="labor-drawer" class=(labor_overlay_cls)
 _="on click[me is event.target] remove .open from me" {
 div class="drawer-panel bg-white h-full w-[800px] max-w-[100vw] flex flex-col shadow-[-8px_0_30px_rgba(0,0,0,0.1)]"
 _="on click halt the event on htmx:afterSettle add .open to #labor-drawer" {
 div class="flex items-center justify-between px-6 py-4 border-b border-border-soft sticky top-0 bg-white z-10" {
 h2 class="flex items-center gap-2 text-base font-semibold text-fg m-0" {
 (icon::bolt_icon("w-5 h-5 text-muted"))
 " BOM 人工成本"
 }
 button type="button" class="w-8 h-8 border-none bg-transparent cursor-pointer text-muted rounded-md grid place-items-center hover:bg-surface hover:text-fg transition-colors"
 _="on click remove .open from closest .drawer-overlay" {
 "×"
 }
 }
 @if auto_open == Some("labor") {
 div class="flex-1 overflow-y-auto p-6" {
 div id="labor-drawer-body"
 hx-get=(labor_drawer_path.to_string())
 hx-trigger="load"
 hx-swap="innerHTML" {
 div class="text-center text-muted py-10" { "加载中..." }
 }
 }
 } @else {
 div class="flex-1 overflow-y-auto p-6" {
 div id="labor-drawer-body" {}
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from closest .drawer-overlay" { "关闭" }
 }
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


/// Build a map from node id → depth. Root nodes (parent_id == 0) have depth 0,
/// others have parent_depth + 1.
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

fn bom_node_row(
 index: usize,
 level: usize,
 has_children: bool,
 node: &BomNode,
 product: Option<&abt_core::master_data::product::model::Product>,
) -> Markup {
 let code = node.product_code.as_deref().or_else(|| product.map(|p| p.product_code.as_str())).unwrap_or("—");
 let name = product.map(|p| p.pdt_name.as_str()).unwrap_or("—");
 let unit = node.unit.as_deref().unwrap_or("—");
 let work_center = node.work_center.as_deref().filter(|s| !s.is_empty()).unwrap_or("—");
 let remark = node.remark.as_deref().filter(|s| !s.is_empty()).unwrap_or("");
 let loss_rate = if node.loss_rate == Decimal::ZERO {
 "—".to_string()
 } else {
 format!("{}%", node.loss_rate)
 };

 let row_class = if level == 1 {
 "bg-[#7030a0] text-white font-medium"
 } else if has_children {
 "bg-[#ff0] text-[#1a1a1a]"
 } else {
 "hover:bg-slate-50"
 };

 html! {
 tr class={(row_class)} {
 td class="px-3 py-2.5 text-center text-xs font-mono opacity-60 border-b border-border-soft" { (index + 1) }
 td class="px-3 py-2.5 text-center border-b border-border-soft" { (level) }
 td class="px-3 py-2.5 font-mono tabular-nums text-sm whitespace-nowrap border-b border-border-soft" { (code) }
 td class="px-3 py-2.5 text-sm border-b border-border-soft" style=(format!("padding-left:{}px", (level - 1) * 24 + 12)) {
 span class="block max-w-[250px] truncate" title=(name) { (name) }
 }
 td class="px-3 py-2.5 text-sm truncate border-b border-border-soft" { (work_center) }
 td class="px-3 py-2.5 font-mono tabular-nums text-sm text-right border-b border-border-soft" { (node.quantity) }
 td class="px-3 py-2.5 text-sm text-center truncate border-b border-border-soft" { (unit) }
 td class="px-3 py-2.5 text-sm text-right border-b border-border-soft" { (loss_rate) }
 td class="px-3 py-2.5 text-sm truncate border-b border-border-soft" { (remark) }
 }
 }
}


// ── Cost Drawer Content ──

fn format_currency(d: Decimal) -> String {
 let val = d.round_dp(6);
 format!("¥{}", val)
}

fn format_amount(unit_price: Decimal, quantity: Decimal) -> String {
 format_currency(unit_price * quantity)
}

fn cost_drawer_content(report: &BomCostReport, temp_prices: &HashMap<i64, String>) -> Markup {
 let temp_price_path = BomCostTempPricePath { id: report.bom_id }.to_string();
 let clear_path = BomCostClearTempPath { id: report.bom_id }.to_string();

 // Calculate material total using actual prices or temp prices
 let has_uncovered_missing = report.material_costs.iter()
 .any(|item| item.unit_price.is_none() && !temp_prices.contains_key(&item.product_id));
 let material_total: Decimal = report.material_costs.iter()
 .map(|item| {
 let price = item.unit_price
 .or_else(|| temp_prices.get(&item.product_id).and_then(|s| s.parse::<Decimal>().ok()));
 price.map(|p| p * item.quantity).unwrap_or(Decimal::ZERO)
 })
 .sum();
 let labor_total: Decimal = report.labor_costs.iter()
 .map(|item| item.unit_price * item.quantity)
 .sum();
 let has_labor_cost_issue = !report.labor_costs.is_empty()
 && report.labor_costs.iter().all(|item| item.unit_price == Decimal::ZERO);

 let all_resolved = !has_uncovered_missing && !has_labor_cost_issue;
 let total_card_class = if all_resolved { "total-ok" } else { "total-warn" };
 let total_sub = if has_uncovered_missing && has_labor_cost_issue {
 "材料缺失单价，人工成本为0"
 } else if has_uncovered_missing {
 "存在缺失单价"
 } else if has_labor_cost_issue {
 "人工成本为0"
 } else if !temp_prices.is_empty() {
 "已完成计算（含临时价格）"
 } else {
 "已完成计算"
 };

 html! {
 div class="space-y-4" {
 // Warning banner
 @if !report.warnings.is_empty() {
 div class="border border-[#fbbf24] bg-[#fffbeb] rounded-md" {
 button type="button" class="flex items-center justify-between w-full py-3 px-4 border-none bg-transparent cursor-pointer text-left hover:bg-[#fef3c7] rounded-md"
 _="on click if next <div/>'s style's display is 'none' then show next <div/> else hide next <div/>" {
 div class="flex items-center gap-2 text-[13px] font-medium text-[#92400e]" {
 (icon::circle_alert_icon("w-4 h-4 text-[#d97706]"))
 span { "部分材料缺失单价（共 " (report.warnings.len()) " 项）" }
 }
 (icon::chevron_down_icon("w-4 h-4"))
 }
 div class="overflow-hidden border-t border-[#fbbf24]" style="display:none" {
 ul class="list-none m-0 p-0 py-3 px-4" {
 @for w in &report.warnings {
 li class="text-[13px] text-[#92400e] py-0.5" { "- " (w) }
 }
 }
 }
 }
 }
 // Product code
 div class="bg-[#f8fafc] rounded-md px-4 py-2.5" {
 p class="text-[13px] text-[#64748b] m-0" { "产品编码：" span class="font-mono font-semibold text-[#0f172a]" { (report.product_code) } }
 }
 // Summary cards
 div class="grid grid-cols-3 gap-3" {
 // Material cost card
 div class="border border-[#e5e7eb] bg-white rounded-md p-4" {
 div class="text-[11px] font-medium text-[#6b7280] mb-1" { "材料成本" }
 div class="text-lg font-bold tabular-nums text-[#2563eb]" { (format_currency(material_total)) }
 div class="text-[11px] text-[#9ca3af] mt-1" { (report.material_costs.len()) " 项材料" }
 }
 // Labor cost card
 div class={(format!("border rounded-md p-4 {}", if has_labor_cost_issue { "border-[#fecaca] bg-[#fef2f2]" } else { "border-[#e5e7eb] bg-white" }))} {
 div class={(format!("text-[11px] font-medium mb-1 {}", if has_labor_cost_issue { "text-[#ef4444]" } else { "text-[#6b7280]" }))} { "人工成本" }
 div class={(format!("text-lg font-bold tabular-nums {}", if has_labor_cost_issue { "text-[#dc2626]" } else { "text-[#2563eb]" }))} { (format_currency(labor_total)) }
 div class={(format!("text-[11px] mt-1 {}", if has_labor_cost_issue { "text-[#f87171]" } else { "text-[#9ca3af]" }))} {
 (report.labor_costs.len()) " 道工序"
 @if has_labor_cost_issue { "（单价为0）" }
 }
 }
 // Total cost card
 div class={(format!("border rounded-md p-4 {}", if all_resolved { "border-[#bfdbfe] bg-[#eff6ff]" } else { "border-[#fde68a] bg-[#fefce8]" }))} {
 div class={(format!("text-[11px] font-medium mb-1 {}", if all_resolved { "text-[#3b82f6]" } else { "text-[#d97706]" }))} { "总成本" }
 @if all_resolved {
 div class={(format!("text-lg font-bold tabular-nums {}", if all_resolved { "text-[#2563eb]" } else { "text-[#d97706]" }))} { (format_currency(material_total + labor_total)) }
 } @else {
 div class="text-lg font-bold text-[#d97706]" { "-" }
 }
 div class={(format!("text-[11px] mt-1 {}", if all_resolved { "text-[#9ca3af]" } else { "text-[#fbbf24]" }))} { (total_sub) }
 }
 }
 // Temp price notice
 @if !temp_prices.is_empty() {
 div class="flex items-center gap-2 bg-[#eff6ff] text-xs text-[#3b82f6] px-3 py-2 rounded-sm" {
 (icon::circle_alert_icon("w-4 h-4"))
 span { "已使用 " strong { (temp_prices.len()) } " 个临时价格" }
 button type="button" class="border-none text-[#3b82f6] text-xs cursor-pointer font-medium bg-transparent"
 hx-delete=(clear_path)
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML" { "清除全部" }
 }
 }
 // Material cost table
 div class="mb-4" {
 div class="text-[13px] font-semibold text-[#374151] mb-2" { "【材料成本】" }
 div class="overflow-hidden border border-[#e5e7eb] rounded-md" {
 table class="w-full border-collapse" {
 thead {
 tr {
 th class="text-left text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "产品名称" }
 th class="text-left text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "产品编码" }
 th class="text-right text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "数量" }
 th class="text-right text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "单价" }
 th class="text-right text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "小计" }
 }
 }
 tbody {
 @for item in &report.material_costs {
 @let effective_price = item.unit_price
 .or_else(|| temp_prices.get(&item.product_id).and_then(|s| s.parse::<Decimal>().ok()));
 @let is_missing = item.unit_price.is_none();
 @let has_temp = is_missing && temp_prices.contains_key(&item.product_id);
 @let tr_class = if is_missing && !has_temp { "bg-[#fee2e2] text-[#991b1b]" } else { "" };
 tr class=(tr_class) {
 td class="px-3 py-2 text-sm font-medium truncate max-w-[160px]" title=(item.product_name) {
 (item.product_name)
 }
 td class="px-3 py-2 text-sm font-mono tabular-nums text-[#6b7280] whitespace-nowrap" { (item.product_code) }
 td class="px-3 py-2 text-sm text-right font-mono tabular-nums whitespace-nowrap" { (item.quantity) }
 td class="px-3 py-2 text-sm text-right whitespace-nowrap" {
 @if let Some(price) = item.unit_price {
 span class="font-mono tabular-nums" { (format_currency(price)) }
 } @else if has_temp {
 span class="inline-flex items-center gap-1.5" {
 span class="font-mono tabular-nums" { (format_currency(effective_price.unwrap_or(Decimal::ZERO))) }
 span class="text-[10px] bg-[#fef3c7] text-[#92400e] px-1.5 py-0.5 rounded" { "临时" }
 }
 form class="inline-flex"
 hx-post=(temp_price_path)
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML" {
 input type="hidden" name="product_id" value=(item.product_id) {}
 input type="hidden" name="temp_price" value="" {}
 button type="submit" class="border-none bg-[#fef2f2] text-[#ef4444] w-[22px] h-[22px] text-sm cursor-pointer grid place-items-center rounded" title="回退"
 _="on click halt the event" {
 "×"
 }
 }
 } @else {
 form class="inline-flex items-center gap-1"
 hx-post=(temp_price_path)
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML" {
 input type="hidden" name="product_id" value=(item.product_id) {}
 input type="text" class="w-[100px] text-xs bg-white outline-none text-right border border-[#e5e7eb] rounded px-2 py-1" name="temp_price"
 placeholder="输入单价"
 _="on click halt the event on focus halt the event" {}
 button type="submit" class="border-none bg-accent text-white w-6 h-6 text-xs cursor-pointer grid place-items-center rounded" title="确认"
 _="on click halt the event" {
 "✓"
 }
 }
 }
 }
 td class="px-3 py-2 text-sm text-right whitespace-nowrap" {
 @if let Some(price) = effective_price {
 @let amt = price * item.quantity;
 @if has_temp {
 span class="font-mono tabular-nums text-[#d97706] font-medium" { (format_currency(amt)) }
 } @else {
 span class="font-mono tabular-nums text-[#2563eb] font-medium" { (format_currency(amt)) }
 }
 } @else {
 span class="text-[#ef4444] font-medium" { "-" }
 }
 }
 }
 }
 }
 }
 }
 div class="flex items-center justify-end gap-2 bg-[#eff6ff] px-4 py-2.5 rounded-md mt-2" {
 span class="text-[13px] font-medium text-[#374151]" { "材料成本合计:" }
 span class="text-base font-bold tabular-nums text-[#2563eb]" id="cost-material-total" { (format_currency(material_total)) }
 }
 }
 // Labor cost table
 div class="mb-4" {
 div class="text-[13px] font-semibold text-[#374151] mb-2" { "【人工成本】" }
 div class="overflow-hidden border border-[#e5e7eb] rounded-md" {
 table class="w-full border-collapse" {
 thead {
 tr {
 th class="text-left text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "工序名称" }
 th class="text-right text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "单价" }
 th class="text-right text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "数量" }
 th class="text-right text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "小计" }
 th class="text-left text-xs font-medium text-[#6b7280] bg-[#f8fafc] px-3 py-2 whitespace-nowrap [border-bottom:1px_solid_#e5e7eb]" { "备注" }
 }
 }
 tbody {
 @if report.labor_costs.is_empty() {
 tr {
 td colspan="5" class="text-center text-muted text-sm py-6" { "暂无人工成本数据" }
 }
 } @else {
 @for item in &report.labor_costs {
 @let is_zero = item.unit_price == Decimal::ZERO;
 tr class=(if is_zero { "bg-[#fee2e2] text-[#991b1b]" } else { "" }) {
 td class="px-3 py-2 text-sm font-medium" { (item.name) }
 td class="px-3 py-2 text-sm text-right whitespace-nowrap" {
 @if is_zero {
 span class="text-[#ef4444] font-medium" { "¥0.000000" }
 } @else {
 span class="font-mono tabular-nums" { (format_currency(item.unit_price)) }
 }
 }
 td class="px-3 py-2 text-sm text-right font-mono tabular-nums whitespace-nowrap" { (item.quantity) }
 td class="px-3 py-2 text-sm text-right whitespace-nowrap" {
 @if is_zero {
 span class="text-[#ef4444] font-medium" { (format_amount(item.unit_price, item.quantity)) }
 } @else {
 span class="font-mono tabular-nums text-[#2563eb] font-medium" { (format_amount(item.unit_price, item.quantity)) }
 }
 }
 td class="px-3 py-2 text-sm text-[#6b7280]" {
 @if item.remark.is_empty() { "—" } @else { (item.remark) }
 }
 }
 }
 }
 }
 }
 }
 div class={(format!("flex items-center justify-end gap-2 px-4 py-2.5 rounded-md mt-2 {}", if has_labor_cost_issue { "bg-[#fef2f2] border border-[#fecaca]" } else { "bg-[#eff6ff]" }))} {
 span class="text-[13px] font-medium text-[#374151]" { "人工成本合计:" }
 span class={(format!("text-base font-bold tabular-nums {}", if has_labor_cost_issue { "text-[#dc2626]" } else { "text-[#2563eb]" }))} {
 (format_currency(labor_total))
 }
 @if has_labor_cost_issue {
 span class="text-[11px] text-[#ef4444] ml-1" { "（所有工序单价为0）" }
 }
 }
 }
 // Total footer
 div class="flex items-center justify-end gap-2 bg-[#f1f5f9] px-4 py-3 rounded-md" {
 @if !all_resolved {
 @let total_hint = if has_uncovered_missing && has_labor_cost_issue {
 "请补全材料单价并设置人工成本"
 } else if has_uncovered_missing {
 "请补全所有材料单价"
 } else {
 "请设置人工成本单价"
 };
 span class="text-[13px] font-medium text-[#d97706]" { (total_hint) }
 } @else {
 span class="text-[13px] font-medium text-[#374151]" { "总成本:" }
 span class="text-lg font-bold tabular-nums text-[#111827]" {
 (format_currency(material_total + labor_total))
 }
 }
 }
 }
 }
}

fn labor_cost_drawer_content(bom_name: &str, report: &BomLaborCostReport) -> Markup {
 let has_issue = !report.items.is_empty()
 && report.items.iter().all(|item| item.unit_price == Decimal::ZERO);

 html! {
 div class="mb-2" {
 p class="text-sm text-fg-2" { "BOM：" span class="font-medium" { (bom_name) } }
 }

 div class="bg-surface border border-border-soft rounded-md p-4 mb-4" {
 div class="text-xs text-muted mb-1" { "人工成本合计" }
 div class="text-xl font-bold text-fg" { (format_currency(report.total_cost)) }
 div class="text-xs text-muted mt-1" {
 (report.items.len()) " 道工序"
 @if has_issue { "（所有工序单价为0）" }
 }
 }

 div class="mb-6" {
 div class="text-[13px] font-semibold text-fg mb-2" { "【人工成本明细】" }
 div class="overflow-x-auto" {
 table class="w-full border-collapse" {
 thead {
 tr {
 th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "工序名称" }
 th class="text-right text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "单价" }
 th class="text-right text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "数量" }
 th class="text-right text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "小计" }
 th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "备注" }
 }
 }
 tbody {
 @if report.items.is_empty() {
 tr {
 td colspan="5" class="text-center text-muted text-sm py-8" { "暂无人工成本数据" }
 }
 } @else {
 @for item in &report.items {
 @let is_zero = item.unit_price == Decimal::ZERO;
 tr class=(if is_zero { "bg-danger-bg" } else { "" }) {
 td class="py-2 px-3 font-medium text-fg border-b border-border-soft" { (item.name) }
 td class="py-2 px-3 text-right border-b border-border-soft" {
 @if is_zero {
 span class="text-[#ef4444] font-medium" { "¥0.000000" }
 } @else {
 span class="font-mono tabular-nums text-fg-2" { (format_currency(item.unit_price)) }
 }
 }
 td class="py-2 px-3 text-right font-mono tabular-nums text-fg-2 border-b border-border-soft" { (item.quantity) }
 td class="py-2 px-3 text-right font-medium border-b border-border-soft" {
 @if is_zero {
 span class="text-[#ef4444]" { (format_amount(item.unit_price, item.quantity)) }
 } @else {
 span class="font-mono tabular-nums text-accent" {
 (format_amount(item.unit_price, item.quantity))
 }
 }
 }
 td class="py-2 px-3 text-muted text-sm border-b border-border-soft" {
 @if item.remark.is_empty() { "—" } @else { (item.remark) }
 }
 }
 }
 }
 }
 }
 }
 div class={(format!("flex items-center justify-between px-4 py-3 rounded-md {}", if has_issue { "bg-danger-bg" } else { "bg-accent-bg" }))} {
 span class="text-sm text-fg-2" { "人工成本合计:" }
 span class={(format!("text-base font-bold {}", if has_issue { "text-[#ef4444]" } else { "text-accent" }))} {
 (format_currency(report.total_cost))
 }
 @if has_issue {
 span class="text-[11px] text-[#ef4444] ml-1" { "（所有工序单价为0）" }
 }
 }
 }
 }
}

pub fn filter_invalid_nodes(nodes: &mut Vec<abt_core::master_data::bom::model::BomNode>, product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>) {
 let invalid_ids: std::collections::HashSet<i64> = nodes.iter()
 .filter(|n| !product_map.contains_key(&n.product_id))
 .map(|n| n.id)
 .collect();
 if invalid_ids.is_empty() { return; }
 // Also remove descendants of invalid nodes
 fn collect_descendants(parent_ids: &std::collections::HashSet<i64>, nodes: &[abt_core::master_data::bom::model::BomNode]) -> std::collections::HashSet<i64> {
 let mut descendants: std::collections::HashSet<i64> = parent_ids.clone();
 let mut changed = true;
 while changed {
 changed = false;
 for n in nodes {
 if !descendants.contains(&n.id) && descendants.contains(&n.parent_id) {
 descendants.insert(n.id);
 changed = true;
 }
 }
 }
 descendants
 }
 let remove_ids = collect_descendants(&invalid_ids, nodes);
 nodes.retain(|n| !remove_ids.contains(&n.id));
}
