use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::bom::BomQueryService;
use abt_core::master_data::bom::model::{Bom, BomNode};
use abt_core::master_data::price::ProductPriceService;
use abt_core::master_data::price::model::{PriceLogEntry, PriceQuery, PriceType};
use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::{*, AcquireChannel, MaterialConsumptionMode};
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::RoutingDetail;
use abt_core::shared::types::PageParams;
use abt_core::wms::stock_ledger::StockLedgerService;
use abt_core::wms::stock_ledger::model::{StockFilter, StockLedger};

use abt_macros::require_permission;

use crate::components::detail::{detail_row, detail_tabs, tab_panel};
use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::product::{ProductDeletePath, ProductDetailPath, ProductEditPath, ProductListPath, ProductUpdatePath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_product_detail(
 path: ProductDetailPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let prod_svc = state.product_service();
 let product = prod_svc.get(&service_ctx, &mut conn, path.id).await?;

 // ── BOM 数据（已发布 BOM + 叶子节点）──
 let bom_svc = state.bom_query_service();
 let bom_id = bom_svc
 .find_published_bom_by_product_code(&service_ctx, &mut conn, &product.product_code)
 .await?;
 let bom = match bom_id {
 Some(id) => bom_svc.get(&service_ctx, &mut conn, id).await.ok(),
 None => None,
 };
 let bom_nodes = match bom_id {
 Some(id) => bom_svc.get_leaf_nodes(&service_ctx, &mut conn, id).await.unwrap_or_default(),
 None => Vec::new(),
 };
 // 解析 BOM 组件的产品名称
 let node_ids: Vec<i64> = bom_nodes.iter().map(|n| n.product_id).collect();
 let node_names: HashMap<i64, String> = if node_ids.is_empty() {
 HashMap::new()
 } else {
 prod_svc
 .get_by_ids(&service_ctx, &mut conn, node_ids)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|p| (p.product_id, p.pdt_name))
 .collect()
 };

 // ── BOM 引用（使用情况）──
 let usage = prod_svc
 .check_product_usage(&service_ctx, &mut conn, path.id, UsageQuery { page: 1, page_size: 50 })
 .await?;

 // ── 工艺路线 ──
 let routing = state
 .routing_service()
 .get_bom_routing(&service_ctx, &mut conn, product.product_code.clone())
 .await
 .ok()
 .flatten();

 // ── 库存台账 ──
 let stock = match state
 .stock_ledger_service()
 .query(&service_ctx, &mut conn, StockFilter { product_id: Some(path.id), ..Default::default() }, 1, 100)
 .await
 {
 Ok(r) => r.items,
 Err(_) => Vec::new(),
 };

 // ── 价格变更记录 ──
 let price_history = match state
 .product_price_service()
 .list_price_history(
 &service_ctx,
 &mut conn,
 PriceQuery { product_id: Some(path.id), price_type: None, keyword: None, date_from: None, date_to: None },
 PageParams::new(1, 50),
 )
 .await
 {
 Ok(r) => r.items,
 Err(_) => Vec::new(),
 };

 let content = product_detail_page(
 &product,
 bom.as_ref(),
 &bom_nodes,
 &node_names,
 &usage.items,
 usage.total,
 routing.as_ref(),
 &stock,
 &price_history,
 );
 let detail_path_str = ProductDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 产品详情", product.pdt_name),
 &claims,
 "md",
 &detail_path_str,
 "主数据管理",
 Some(&product.product_code),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "update")]
pub async fn get_product_edit(
 path: ProductEditPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.product_service();

 let product = svc.get(&service_ctx, &mut conn, path.id).await?;
 let title = format!("{} - 编辑产品", product.pdt_name);
 let edit_path_str = ProductEditPath { id: path.id }.to_string();
 let content = product_edit_page(&product);
 let page_html = admin_page(
 is_htmx,
 &title,
 &claims,
 "md",
 &edit_path_str,
 "主数据管理",
 Some(&title),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ProductEditForm {
 pub name: String,
 pub unit: String,
 pub specification: String,
 pub acquire_channel: Option<String>,
 pub external_code: Option<String>,
 pub owner_department_id: Option<String>,
 pub old_code: Option<String>,
 pub remark: Option<String>,
 pub material_consumption_mode: Option<String>,
}

#[require_permission("PRODUCT", "update")]
pub async fn update_product(
 path: ProductUpdatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ProductEditForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.product_service();

 let owner_department_id = form
 .owner_department_id
 .as_deref()
 .and_then(|s| if s.is_empty() { None } else { s.parse::<i64>().ok() });

 // 将中文获取途径映射为枚举值
 let acquire_channel = match form.acquire_channel.as_deref() {
 Some("自制") => Some(AcquireChannel::SelfProduced),
 Some("采购") => Some(AcquireChannel::Purchased),
 Some("委外") => Some(AcquireChannel::Outsourced),
 _ => None, // 不修改，保持原值
 };

 let material_consumption_mode = match form.material_consumption_mode.as_deref() {
 Some("picking") => MaterialConsumptionMode::Picking,
 _ => MaterialConsumptionMode::Backflush,
 };

 let req = UpdateProductReq {
 name: Some(form.name),
 unit: Some(form.unit),
 acquire_channel,
 external_code: form.external_code.filter(|s| !s.is_empty()),
 owner_department_id,
 meta: Some(ProductMeta {
 specification: form.specification,
 old_code: form.old_code.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 material_consumption_mode,
 over_completion_tolerance: None,
 }),
 };

 svc.update(&service_ctx, &mut conn, path.id, req).await?;

 let redirect = ProductDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Detail Page (5-Tab) ──

#[allow(clippy::too_many_arguments)]
fn product_detail_page(
 product: &Product,
 bom: Option<&Bom>,
 bom_nodes: &[BomNode],
 node_names: &HashMap<i64, String>,
 usage: &[UsageEntry],
 usage_total: u64,
 routing: Option<&RoutingDetail>,
 stock: &[StockLedger],
 price_history: &[PriceLogEntry],
) -> Markup {
 let list_path = ProductListPath;
 let edit_path = ProductEditPath { id: product.product_id };
 let delete_path = ProductDeletePath { id: product.product_id };

 let (status_label, status_class) = status_display(product.status);

 html! {
 div {
 // ── Detail Top ──
 div class="flex justify-between items-start" {
 div class="flex items-center gap-5" {
 div class="w-10 h-10 grid place-items-center rounded-full bg-accent text-white font-semibold shrink-0 select-none" {
 (icon::box_icon("w-5 h-5"))
 }
 div {
 h1 class="text-xl font-bold" {
 (product.pdt_name)
 " "
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 div class="flex gap-4 text-muted text-xs" {
 span { "编码: " (product.product_code) }
 span { "单位: " (product.unit) }
 @if let Some(dt) = product.created_at {
 span { "创建: " (dt.format("%Y-%m-%d")) }
 }
 }
 }
 }
 div class="flex gap-3" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{list_path}?restore=true")) {
 (icon::arrow_left_icon("w-4 h-4"))
 " 返回列表"
 }
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(edit_path) {
 (icon::edit_icon("w-4 h-4"))
 " 编辑"
 }
 }
 }

 // ── Tab Bar ──
 (detail_tabs("info", &[
 ("info", "基本信息"),
 ("config", "生产配置"),
 ("bom", "BOM"),
 ("stock", "库存"),
 ("history", "变更记录"),
 ]))

 // ── Tab 1: 基本信息 ──
 (tab_panel("info", true, tab_basic_info(product, status_label, status_class)))

 // ── Tab 2: 生产配置 ──
 (tab_panel("config", false, tab_production_config(
 product, bom, bom_nodes.len(), routing, usage, usage_total,
 )))

 // ── Tab 3: BOM ──
 (tab_panel("bom", false, tab_bom(bom, bom_nodes, node_names)))

 // ── Tab 4: 库存 ──
 (tab_panel("stock", false, tab_stock(stock)))

 // ── Tab 5: 变更记录 ──
 (tab_panel("history", false, tab_history(price_history)))

 // ── Delete form (hx-confirm) ──
 form id="delete-product-form" class="hidden"
 hx-post=(delete_path.to_string())
 hx-confirm=(format!("确定要删除产品「{}」吗？此操作不可撤销。", product.pdt_name))
 hx-target="closest div" {}
 }
 }
}

// ── Tab: 基本信息 ──

fn tab_basic_info(product: &Product, status_label: &'static str, status_class: &'static str) -> Markup {
 html! {
 div class="grid gap-5" {
 // 基本信息
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "基本信息" }
 (detail_row("产品编码", html! { span class="font-mono tabular-nums" { (product.product_code) } }))
 (detail_row("产品名称", html! { (product.pdt_name) }))
 (detail_row("规格型号", html! {
 @if product.meta.specification.is_empty() { "—" } @else { (&product.meta.specification) }
 }))
 (detail_row("计量单位", html! { (product.unit) }))
 (detail_row("获取途径", html! { (acquire_channel_label(product.acquire_channel)) }))
 (detail_row("产品状态", html! {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }))
 (detail_row("创建时间", html! {
 @if let Some(dt) = product.created_at { (dt.format("%Y-%m-%d %H:%M")) } @else { "—" }
 }))
 (detail_row("更新时间", html! {
 @if let Some(dt) = product.updated_at { (dt.format("%Y-%m-%d %H:%M")) } @else { "—" }
 }))
 }

 // 分类与归属
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "分类与归属" }
 (detail_row("外部编码", html! {
 (product.external_code.as_deref().unwrap_or("—"))
 }))
 (detail_row("旧编码", html! {
 (product.meta.old_code.as_deref().unwrap_or("—"))
 }))
 (detail_row("归属部门", html! { "—" }))
 (detail_row("备注", html! {
 @if let Some(r) = &product.meta.remark {
 (r)
 } @else {
 "—"
 }
 }))
 }

 // 规格参数
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "规格参数" }
 @if product.meta.specification.is_empty() {
 div class="text-center p-6 text-muted text-sm" { "暂无规格参数" }
 } @else {
 @for line in product.meta.specification.lines() {
 div class="flex py-2 text-sm" {
 span class="detail-value" { (line) }
 }
 }
 }
 }
 }
 }
}

// ── Tab: 生产配置 ──

#[allow(clippy::too_many_arguments)]
fn tab_production_config(
 product: &Product,
 bom: Option<&Bom>,
 bom_node_count: usize,
 routing: Option<&RoutingDetail>,
 usage: &[UsageEntry],
 usage_total: u64,
) -> Markup {
 let mode = product.meta.material_consumption_mode;
 html! {
 // ── Section: BOM 与工艺路线 ──
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "BOM 与工艺路线" }
 div class="grid gap-5" {
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "当前 BOM" }
 span class="text-sm text-fg font-medium flex items-center gap-2" {
 @if let Some(b) = bom {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" { (b.bom_name) " V"(b.version) }
 } @else {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted" { "未关联" }
 }
 }
 }
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "工艺路线" }
 span class="text-sm text-fg font-medium flex items-center gap-2" {
 @if let Some(rd) = routing {
 (rd.routing.name)
 " "
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" { (rd.steps.len()) " 工序" }
 } @else {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted" { "未关联" }
 }
 }
 }
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "工作中心" }
 span class="text-sm text-fg font-medium flex items-center gap-2" { "—" }
 }
 }
 }

 // ── Section: 物料消耗配置 ──
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "物料消耗配置" }
 div class="grid gap-5" {
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "物料消耗模式" }
 div class="text-sm text-fg font-medium flex items-center gap-2" {
 div class="inline-flex bg-surface border border-border rounded-full gap-[2px]" {
 span class=(if mode == MaterialConsumptionMode::Backflush { "px-3 py-1 text-xs rounded-full bg-accent text-accent-on" } else { "px-3 py-1 text-xs rounded-full bg-surface text-muted" }) { "倒冲" }
 span class=(if mode == MaterialConsumptionMode::Picking { "px-3 py-1 text-xs rounded-full bg-accent text-accent-on" } else { "px-3 py-1 text-xs rounded-full bg-surface text-muted" }) { "领料" }
 }
 }
 }
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "超额完工容差" }
 span class="text-sm text-fg font-medium flex items-center gap-2" {
 span class="font-mono tabular-nums" {
 @if let Some(t) = &product.meta.over_completion_tolerance {
 (*t * Decimal::from(100)) "%"
 } @else {
 "5%（默认）"
 }
 }
 }
 }
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "模式说明" }
 span class="text-sm text-fg font-medium flex items-center gap-2" {
 @match mode {
 MaterialConsumptionMode::Backflush => { "倒冲模式：完工入库时按 BOM 自动扣减原材料，不生成领料单" }
 MaterialConsumptionMode::Picking => { "领料模式：下达时生成领料单，手动领料出库" }
 }
 }
 }
 }
 }

 // ── Section: 生产参数 ──
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "生产参数" }
 div class="grid gap-5" {
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "默认仓库" }
 span class="text-sm text-fg font-medium flex items-center gap-2" { "—" }
 }
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "BOM 组件数" }
 span class="text-sm text-fg font-medium flex items-center gap-2 font-mono tabular-nums" { (bom_node_count) }
 }
 div class="flex flex-col gap-[3px]" {
 span class="text-xs text-muted font-medium" { "工序总数" }
 span class="text-sm text-fg font-medium flex items-center gap-2 font-mono tabular-nums" { (routing.map_or(0, |rd| rd.steps.len())) }
 }
 }
 }

 // ── 使用情况（BOM 引用）──
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" {
 span { "使用情况（BOM 引用）" }
 " "
 span class="text-xs text-muted font-medium" { "该产品被以下 BOM 引用" }
 }
 @if usage.is_empty() {
 div class="text-center p-6 text-muted text-sm" { "该产品暂未被任何 BOM 引用" }
 } @else {
 table class="data-table" {
 thead {
 tr {
 th { "父件产品" }
 th { "BOM 名称" }
 th { "版本" }
 th { "用量" }
 th { "BOM 状态" }
 }
 }
 tbody {
 @for entry in usage {
 tr {
 td { (entry.parent_product_name.as_deref().unwrap_or("—")) }
 td { (entry.source_name) }
 td {
 @if let Some(v) = entry.bom_version { "V"(v) } @else { "—" }
 }
 td {
 @if let Some(q) = entry.quantity {
 (q)
 @if let Some(u) = &entry.node_unit { " " (u) }
 } @else {
 "—"
 }
 }
 td {
 @if entry.bom_status == Some(2) {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" { "已发布" }
 } @else if entry.bom_status == Some(1) {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted" { "草稿" }
 } @else {
 "—"
 }
 }
 }
 }
 }
 }
 @if (usage_total as usize) > usage.len() {
 div class="text-xs text-muted font-medium" { "共 " (usage_total) " 条引用记录" }
 }
 }
 }
 }
}

// ── Tab: BOM ──

fn tab_bom(bom: Option<&Bom>, bom_nodes: &[BomNode], node_names: &HashMap<i64, String>) -> Markup {
 html! {
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" {
 span { "BOM 组件清单" }
 @if let Some(b) = bom {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" { "已发布 V"(b.version) }
 }
 }
 @if bom_nodes.is_empty() {
 div class="text-center p-6 text-muted text-sm" {
 p { "该产品暂无已发布 BOM 组件" }
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href="/admin/md/boms" { "前往维护 BOM" }
 }
 } @else {
 table class="data-table" {
 thead {
 tr {
 th { "物料编码" }
 th { "物料名称" }
 th { "用量" }
 th { "单位" }
 }
 }
 tbody {
 @for node in bom_nodes {
 tr {
 td { span class="font-mono tabular-nums" { (node.product_code.as_deref().unwrap_or("—")) } }
 td { (node_names.get(&node.product_id).map(|s| s.as_str()).unwrap_or("—")) }
 td class="font-mono tabular-nums" { (node.quantity) }
 td { (node.unit.as_deref().unwrap_or("—")) }
 }
 }
 }
 }
 }
 }
 }
}

// ── Tab: 库存 ──

fn tab_stock(stock: &[StockLedger]) -> Markup {
 html! {
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "库存分布" }
 @if stock.is_empty() {
 div class="text-center p-6 text-muted text-sm" { "该产品暂无库存记录" }
 } @else {
 table class="data-table" {
 thead {
 tr {
 th { "仓库" }
 th { "库位" }
 th { "数量" }
 th { "可用" }
 th { "预留" }
 }
 }
 tbody {
 @for s in stock {
 tr {
 td class="font-mono tabular-nums" { "#" (s.warehouse_id) }
 td class="font-mono tabular-nums" { "#" (s.bin_id) }
 td class="font-mono tabular-nums" { (s.quantity) }
 td class="font-mono tabular-nums" { (s.available_qty) }
 td class="font-mono tabular-nums" { (s.reserved_qty) }
 }
 }
 }
 }
 }
 }
 }
}

// ── Tab: 变更记录 ──

fn tab_history(price_history: &[PriceLogEntry]) -> Markup {
 html! {
 div class="bg-white border border-border-soft rounded p-5" {
 div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "价格变更记录" }
 @if price_history.is_empty() {
 div class="text-center p-6 text-muted text-sm" { "暂无价格变更记录" }
 } @else {
 table class="data-table" {
 thead {
 tr {
 th { "时间" }
 th { "类型" }
 th { "原价" }
 th { "新价" }
 th { "操作人" }
 }
 }
 tbody {
 @for e in price_history {
 tr {
 td { (e.created_at.format("%Y-%m-%d %H:%M")) }
 td { (price_type_label(e.price_type)) }
 td class="font-mono tabular-nums" {
 @if let Some(old) = e.old_price { "¥" (format!("{:.4}", old)) } @else { "—" }
 }
 td class="font-mono tabular-nums" { "¥" (format!("{:.4}", e.new_price)) }
 td {
 @if let Some(oid) = e.operator_id { "#" (oid) } @else { "—" }
 }
 }
 }
 }
 }
 }
 }
 }
}

// ── Edit Page ──

fn product_edit_page(product: &Product) -> Markup {
 let update_path = ProductUpdatePath { id: product.product_id };
 let detail_path = ProductDetailPath { id: product.product_id };

 let acquire_val = match product.acquire_channel {
 AcquireChannel::SelfProduced => "自制",
 AcquireChannel::Purchased => "采购",
 AcquireChannel::Outsourced => "委外",
 AcquireChannel::NonInventory => "非库存",
 AcquireChannel::Legacy => "历史遗留",
 };
 let external_code_val = product.external_code.as_deref().unwrap_or("");
 let old_code_val = product.meta.old_code.as_deref().unwrap_or("");
 let remark_val = product.meta.remark.as_deref().unwrap_or("");
 let mcm_val = match product.meta.material_consumption_mode {
 MaterialConsumptionMode::Backflush => "backflush",
 MaterialConsumptionMode::Picking => "picking",
 };

 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(detail_path) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回产品详情"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "编辑产品" }
 }

 form id="product-edit-form"
 hx-post=(update_path)
 hx-swap="none" {

 // ── Section: 基本信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "产品名称 " span class="text-danger" { "*" } }
 input type="text" name="name" required placeholder="请输入产品名称" value=(product.pdt_name) {}
 }
 div class="form-field" {
 label { "产品编码" }
 input type="text" value=(product.product_code) readonly
 class="bg-surface text-muted" {}
 }
 div class="form-field" {
 label { "规格型号" }
 input type="text" name="specification" placeholder="请输入规格型号" value=(product.meta.specification) {}
 }
 div class="form-field" {
 label { "计量单位 " span class="text-danger" { "*" } }
 select name="unit" required {
 option value="个" selected[product.unit == "个"] { "个" }
 option value="件" selected[product.unit == "件"] { "件" }
 option value="台" selected[product.unit == "台"] { "台" }
 option value="套" selected[product.unit == "套"] { "套" }
 option value="批" selected[product.unit == "批"] { "批" }
 option value="kg" selected[product.unit == "kg" || product.unit == "千克"] { "千克 (kg)" }
 option value="g" selected[product.unit == "g" || product.unit == "克"] { "克 (g)" }
 option value="m" selected[product.unit == "m" || product.unit == "米"] { "米 (m)" }
 option value="cm" selected[product.unit == "cm" || product.unit == "厘米"] { "厘米 (cm)" }
 option value="L" selected[product.unit == "L" || product.unit == "升"] { "升 (L)" }
 option value="卷" selected[product.unit == "卷"] { "卷" }
 option value="包" selected[product.unit == "包"] { "包" }
 option value="箱" selected[product.unit == "箱"] { "箱" }
 option value="根" selected[product.unit == "根"] { "根" }
 option value="块" selected[product.unit == "块"] { "块" }
 option value="片" selected[product.unit == "片"] { "片" }
 option value="张" selected[product.unit == "张"] { "张" }
 option value="条" selected[product.unit == "条"] { "条" }
 }
 }
 div class="form-field" {
 label { "获取途径" }
 select name="acquire_channel" {
 option value="采购" selected[acquire_val == "采购"] { "采购" }
 option value="自制" selected[acquire_val == "自制"] { "自制" }
 option value="委外" selected[acquire_val == "委外"] { "委外" }
 }
 }
 div class="form-field" {
 label { "物料消耗模式" }
 select name="material_consumption_mode" {
 option value="backflush" selected[mcm_val == "backflush"] { "倒冲 (backflush)" }
 option value="picking" selected[mcm_val == "picking"] { "领料 (picking)" }
 }
 }
 div class="form-field" {
 label { "外部编码" }
 input type="text" name="external_code" placeholder="请输入外部编码" value=(external_code_val) {}
 }
 }
 }

 // ── Section: 分类与归属 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "分类与归属" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "归属部门" }
 select name="owner_department_id" {
 option value="" { "-- 请选择 --" }
 }
 }
 div class="form-field" {
 label { "旧编码" }
 input type="text" name="old_code" placeholder="请输入旧编码" value=(old_code_val) {}
 }
 }
 }

 // ── Section: 其他信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "其他信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field field-full" {
 label { "备注" }
 textarea name="remark" placeholder="请输入备注信息…"
 class="w-full resize-y" class="min-h-[80px]" {
 (remark_val)
 }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(detail_path) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "保存修改"
 }
 }
 }
 }
 }
}


fn status_display(status: ProductStatus) -> (&'static str, &'static str) {
 match status {
 ProductStatus::Active => ("在用", "status-accepted"),
 ProductStatus::Inactive => ("停用", "status-draft"),
 ProductStatus::Obsolete => ("作废", "status-rejected"),
 }
}

fn acquire_channel_label(ch: AcquireChannel) -> &'static str {
 match ch {
 AcquireChannel::SelfProduced => "自制",
 AcquireChannel::Purchased => "采购",
 AcquireChannel::Outsourced => "委外",
 AcquireChannel::NonInventory => "非库存",
 AcquireChannel::Legacy => "历史遗留",
 }
}

fn price_type_label(pt: PriceType) -> &'static str {
 match pt {
 PriceType::Purchase => "采购价",
 PriceType::Sales => "销售价",
 PriceType::StandardCost => "标准成本",
 }
}
