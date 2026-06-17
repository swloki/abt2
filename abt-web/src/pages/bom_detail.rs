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

// ── Handlers ──

#[require_permission("BOM", "read")]
pub async fn get_bom_detail(
 path: BomDetailPath,
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

 let content = bom_detail_page(&bom, &product_map, can_view_cost, can_view_labor_cost, can_edit, can_delete);
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
 div class="customer-inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none" style="background:var(--color-primary-light,#e0e7ff)" {
 (icon::clipboard_list_icon("w-5 h-5"))
 }
 div {
 h1 class="text-xl font-bold" {
 (bom.bom_name)
 " "
 span class="bg-[#e6f4ff] text-accent rounded-full text-[11px] font-medium" { "v" (bom.version) }
 " "
 span class=(format!("status-pill {status_class}")) { (status_label) }
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
 div class="flex gap-3" {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{list_path}?restore=true")) {
 (icon::arrow_left_icon("w-4 h-4"))
 " 返回列表"
 }
 @if can_view_cost {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-get=(cost_drawer_path.to_string())
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .open to #cost-drawer" {
 (icon::currency_icon("w-4 h-4"))
 " 查看成本"
 }
 } @else if can_view_labor_cost {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-get=(labor_drawer_path.to_string())
 hx-target="#labor-drawer-body"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .open to #labor-drawer" {
 (icon::bolt_icon("w-4 h-4"))
 " 查看人工成本"
 }
 }
 @if can_edit {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(BomEditPath { id: bom.bom_id }) {
 (icon::edit_icon("w-4 h-4"))
 " 编辑"
 }
 }
 @if can_edit && is_draft {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-confirm="确定要发布此 BOM 吗？发布后将无法修改。"
 hx-post=(publish_path.to_string())
 hx-swap="none" {
 (icon::check_circle_icon("w-4 h-4"))
 " 发布"
 }
 }
 @if can_delete {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90-ghost"
 hx-confirm=(format!("确定要删除 BOM {} 吗？此操作不可撤销。", bom.bom_name))
 hx-post=(delete_path.to_string())
 hx-target="body"
 hx-swap="outerHTML" {
 (icon::trash_icon("w-4 h-4"))
 " 删除"
 }
 }
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-post=(format!("{}/bom?bom_id={}", crate::routes::excel::EXPORT_START_PATH, bom.bom_id))
 hx-confirm="确定要导出 BOM 吗？"
 hx-swap="none" {
 (icon::download_icon("w-4 h-4"))
 " 导出 BOM"
 }
 }
 }

 // ── BOM结构 ──
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 span { "BOM结构" }
 span style="color:var(--text-tertiary);font-weight:400;font-size:12px" {
 "（共 " (node_count) " 个节点）"
 }
 }
 @if bom.bom_detail.nodes.is_empty() {
 div class="text-center p-6 text-muted text-sm" { "暂无BOM节点" }
 } @else {
 table class="w-full text-[13px]" style="table-layout:fixed" {
 thead {
 tr {
 th style="width:40px" { "编号" }
 th style="width:40px" { "层级" }
 th style="width:120px" { "产品编码" }
 th class="bom-col-name" { "产品" }
 th style="width:100px" { "工作中心" }
 th style="width:80px" { "数量" }
 th style="width:60px" { "单位" }
 th style="width:50px" { "损耗率" }
 th style="width:90px" { "备注" }
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

 @if can_view_cost {
 // ── Cost Drawer (wider: 1000px) ──
 div id="cost-drawer" class="fixed z-[1000] flex justify-end opacity-0"
 _="on click remove .open from #cost-drawer" {
 div class="bg-white h-full w-[420px] flex flex-col" style="max-width:1000px;width:100%" onclick="event.stopPropagation()" {
 div class="flex items-center justify-between px-6 py-4 [border-bottom:1px_solid_var(--border-soft)]" {
 h2 { (icon::currency_icon("w-5 h-5")) " BOM成本报告" }
 button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
 _="on click remove .open from #cost-drawer" { "×" }
 }
 div class="flex-1 overflow-y-auto p-6" {
 div id="cost-drawer-body" {
 div style="text-align:center;padding:40px;color:var(--muted)" { "加载中..." }
 }
 }
 div class="px-6 py-4 [border-top:1px_solid_var(--border-soft)] flex justify-end gap-3" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from #cost-drawer" { "关闭" }
 }
 }
 }
 } @else if can_view_labor_cost {
 // ── Labor Cost Drawer (wider: 800px) ──
 div id="labor-drawer" class="fixed z-[1000] flex justify-end opacity-0"
 _="on click remove .open from #labor-drawer" {
 div class="bg-white h-full w-[420px] flex flex-col" style="max-width:800px;width:100%" onclick="event.stopPropagation()" {
 div class="flex items-center justify-between px-6 py-4 [border-bottom:1px_solid_var(--border-soft)]" {
 h2 { (icon::bolt_icon("w-5 h-5")) " BOM 人工成本" }
 button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
 _="on click remove .open from #labor-drawer" { "×" }
 }
 div class="flex-1 overflow-y-auto p-6" {
 div id="labor-drawer-body" {
 div style="text-align:center;padding:40px;color:var(--muted)" { "加载中..." }
 }
 }
 div class="px-6 py-4 [border-top:1px_solid_var(--border-soft)] flex justify-end gap-3" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from #labor-drawer" { "关闭" }
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

 // Row background class (matching old code getNodeRowStyle)
 let row_class = if level == 1 {
 "bom-row-level-0"
 } else if has_children {
 "bom-row-level-1"
 } else {
 "bom-row-level-default"
 };

 html! {
 tr class=(row_class) {
 td style="text-align:center" { (index + 1) }
 td style="text-align:center" { (level) }
 td class="font-mono tabular-nums" { (code) }
 td class="bom-col-name" { (name) }
 td { (work_center) }
 td class="font-mono tabular-nums" style="text-align:right" { (node.quantity) }
 td { (unit) }
 td style="text-align:right" { (loss_rate) }
 td style="color:var(--muted)" { (remark) }
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
 div {
 // Warning banner
 @if !report.warnings.is_empty() {
 div class="cost-warning-banner" {
 button type="button" class="flex items-center justify-between w-full border-none cursor-pointer text-left"
 _="on click toggle .show on next <div/>" {
 div class="warning-left" {
 (icon::circle_alert_icon("w-4 h-4"))
 span { "部分材料缺失单价（共 " (report.warnings.len()) " 项）" }
 }
 (icon::chevron_down_icon("w-4 h-4"))
 }
 div class="grid grid-rows-[0fr] transition-[grid-template-rows] duration-300" {
 ul {
 @for w in &report.warnings {
 li { "- " (w) }
 }
 }
 }
 }
 }
 // Product code
 div class="cost-product-code" {
 p { "产品编码：" span { (report.product_code) } }
 }
 // Summary cards
 div class="grid gap-[12px]" {
 div class="cost-summary-card primary" {
 div class="card-label" { "材料成本" }
 div class="card-value" { (format_currency(material_total)) }
 div class="card-sub" { (report.material_costs.len()) " 项材料" }
 }
 div class={"cost-summary-card " (if has_labor_cost_issue { "danger" } else { "" })} {
 div class="card-label" { "人工成本" }
 div class="card-value" { (format_currency(labor_total)) }
 div class="card-sub" {
 (report.labor_costs.len()) " 道工序"
 @if has_labor_cost_issue { "（单价为0）" }
 }
 }
 div class={"cost-summary-card " (total_card_class)} {
 div class="card-label" { "总成本" }
 @if all_resolved {
 div class="card-value" { (format_currency(material_total + labor_total)) }
 } @else {
 div class="card-value" { "-" }
 }
 div class="card-sub" { (total_sub) }
 }
 }
 // Temp price notice
 @if !temp_prices.is_empty() {
 div class="flex items-center gap-[8px] bg-[#eff6ff] text-[12px] text-[#3b82f6]" {
 (icon::circle_alert_icon("w-4 h-4"))
 span { "已使用 " strong { (temp_prices.len()) } " 个临时价格" }
 button type="button" class="border-none text-[#3b82f6] text-[12px] cursor-pointer font-medium"
 hx-delete=(clear_path)
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML" { "清除全部" }
 }
 }
 // Material cost table
 div class="mb-6" {
 div class="text-[13px] font-semibold text-[#374151]" { "【材料成本】" }
 table class="w-full overflow-hidden" {
 thead {
 tr {
 th class="col-name" { "产品名称" }
 th { "产品编码" }
 th class="text-right" { "数量" }
 th class="text-right" { "单价" }
 th class="text-right" { "小计" }
 }
 }
 tbody {
 @for item in &report.material_costs {
 @let effective_price = item.unit_price
 .or_else(|| temp_prices.get(&item.product_id).and_then(|s| s.parse::<Decimal>().ok()));
 @let is_missing = item.unit_price.is_none();
 @let has_temp = is_missing && temp_prices.contains_key(&item.product_id);
 @let tr_class = if is_missing && !has_temp { "row-danger" } else { "" };
 tr class=(tr_class) {
 td class="cell-name font-mono tabular-nums" title=(item.product_name) {
 (item.product_name)
 }
 td class="font-mono tabular-nums cell-code" { (item.product_code) }
 td class="text-right font-mono tabular-nums" { (item.quantity) }
 td class="text-right" {
 @if let Some(price) = item.unit_price {
 span class="font-mono tabular-nums" { (format_currency(price)) }
 } @else if has_temp {
 span class="inline-flex items-center gap-[6px]" {
 span { (format_currency(effective_price.unwrap_or(Decimal::ZERO))) }
 span class="temp-tag" { "临时" }
 }
 form style="display:inline"
 hx-post=(temp_price_path)
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML" {
 input type="hidden" name="product_id" value=(item.product_id) {}
 input type="hidden" name="temp_price" value="" {}
 button type="submit" class="border-none bg-[#fef2f2] text-[#ef4444] w-[22px] h-[22px] text-[14px] cursor-pointer place-items-center" title="回退"
 onclick="event.stopPropagation()" {
 "×"
 }
 }
 } @else {
 form class="inline-flex items-center gap-[4px]"
 hx-post=(temp_price_path)
 hx-target="#cost-drawer-body"
 hx-swap="innerHTML" {
 input type="hidden" name="product_id" value=(item.product_id) {}
 input type="text" class="w-[100px] text-[12px] bg-white outline-none text-right" name="temp_price"
 placeholder="输入单价"
 onfocus="event.stopPropagation()"
 onclick="event.stopPropagation()" {}
 button type="submit" class="border-none bg-[#2563eb] text-[#fff] w-[24px] h-[24px] text-[13px] cursor-pointer grid place-items-center" title="确认"
 onclick="event.stopPropagation()" {
 "✓"
 }
 }
 }
 }
 td class="text-right cell-amount" {
 @if let Some(price) = effective_price {
 @let amt = price * item.quantity;
 @if has_temp {
 span class="font-mono tabular-nums amount-warn" { (format_currency(amt)) }
 } @else {
 span class="font-mono tabular-nums amount-primary" { (format_currency(amt)) }
 }
 } @else {
 span class="missing-price" { "-" }
 }
 }
 }
 }
 }
 }
 div class="flex items-center justify-end gap-[8px] bg-blue" {
 span class="footer-label" { "材料成本合计:" }
 span class="footer-value blue" id="cost-material-total" { (format_currency(material_total)) }
 }
 }
 // Labor cost table
 div class="mb-6" {
 div class="text-[13px] font-semibold text-[#374151]" { "【人工成本】" }
 table class="w-full overflow-hidden" {
 thead {
 tr {
 th { "工序名称" }
 th class="text-right" { "单价" }
 th class="text-right" { "数量" }
 th class="text-right" { "小计" }
 th { "备注" }
 }
 }
 tbody {
 @if report.labor_costs.is_empty() {
 tr {
 td colspan="5" class="text-center text-muted text-sm" { "暂无人工成本数据" }
 }
 } @else {
 @for item in &report.labor_costs {
 @let is_zero = item.unit_price == Decimal::ZERO;
 tr class=(if is_zero { "row-danger" } else { "" }) {
 td class="cell-bold" { (item.name) }
 td class="text-right" {
 @if is_zero {
 span class="price-zero" { "¥0.000000" }
 } @else {
 span class="font-mono tabular-nums" { (format_currency(item.unit_price)) }
 }
 }
 td class="text-right font-mono tabular-nums" { (item.quantity) }
 td class="text-right cell-amount" {
 @if is_zero {
 span class="amount-danger" { (format_amount(item.unit_price, item.quantity)) }
 } @else {
 span class="font-mono tabular-nums amount-primary" { (format_amount(item.unit_price, item.quantity)) }
 }
 }
 td class="cell-remark" {
 @if item.remark.is_empty() { "—" } @else { (item.remark) }
 }
 }
 }
 }
 }
 }
 div class={"cost-drawer-footer " (if has_labor_cost_issue { "bg-red" } else { "bg-blue" })} {
 span class="footer-label" { "人工成本合计:" }
 span class={"footer-value " (if has_labor_cost_issue { "red" } else { "blue" })} {
 (format_currency(labor_total))
 }
 @if has_labor_cost_issue {
 span class="hint-labor" { "（所有工序单价为0）" }
 }
 }
 }
 // Total footer
 div class="flex items-center justify-end gap-[8px] bg-gray total-footer" {
 @if !all_resolved {
 @let total_hint = if has_uncovered_missing && has_labor_cost_issue {
 "请补全材料单价并设置人工成本"
 } else if has_uncovered_missing {
 "请补全所有材料单价"
 } else {
 "请设置人工成本单价"
 };
 span class="hint-warn" { (total_hint) }
 } @else {
 span class="footer-label" { "总成本:" }
 span class="footer-value dark value-lg" {
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
 // Product code
 div class="cost-product-code" {
 p { "BOM：" span style="font-weight:500" { (bom_name) } }
 }

 // Labor cost summary card
 div class="labor-summary-card" {
 div class="card-label" { "人工成本合计" }
 div class="card-value" { (format_currency(report.total_cost)) }
 div class="card-sub" {
 (report.items.len()) " 道工序"
 @if has_issue { "（所有工序单价为0）" }
 }
 }

 // Detail table
 div style="margin-bottom:24px" {
 div class="text-[13px] font-semibold text-[#374151]" { "【人工成本明细】" }
 table class="w-full overflow-hidden" {
 thead {
 tr {
 th { "工序名称" }
 th class="text-right" { "单价" }
 th class="text-right" { "数量" }
 th class="text-right" { "小计" }
 th { "备注" }
 }
 }
 tbody {
 @if report.items.is_empty() {
 tr {
 td colspan="5" style="text-align:center;padding:32px;color:#9ca3af" { "暂无人工成本数据" }
 }
 } @else {
 @for item in &report.items {
 @let is_zero = item.unit_price == Decimal::ZERO;
 tr class=(if is_zero { "row-danger" } else { "" }) {
 td style="font-weight:500" { (item.name) }
 td class="text-right" {
 @if is_zero {
 span style="color:#ef4444;font-weight:500" { "¥0.000000" }
 } @else {
 span class="font-mono tabular-nums" { (format_currency(item.unit_price)) }
 }
 }
 td class="text-right font-mono tabular-nums" { (item.quantity) }
 td class="text-right" style="font-weight:500" {
 @if is_zero {
 span style="color:#ef4444" { (format_amount(item.unit_price, item.quantity)) }
 } @else {
 span class="font-mono tabular-nums" style="color:#2563eb" {
 (format_amount(item.unit_price, item.quantity))
 }
 }
 }
 td style="color:#6b7280" {
 @if item.remark.is_empty() { "—" } @else { (item.remark) }
 }
 }
 }
 }
 }
 }
 div class={"cost-drawer-footer " (if has_issue { "bg-red" } else { "bg-blue" })} {
 span class="footer-label" { "人工成本合计:" }
 span class={"footer-value " (if has_issue { "red" } else { "blue" })} {
 (format_currency(report.total_cost))
 }
 @if has_issue {
 span style="font-size:11px;color:#ef4444;margin-left:4px" { "（所有工序单价为0）" }
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
