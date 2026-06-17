//! MES 生产需求池列表页 — 按物料聚合展示自制需求

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use chrono::NaiveDate;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::mes::demand_handler::{
 DemandPoolQuery, DemandSummary, MaterialAggQuery, MaterialAggSummary, MesDemandService,
};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_demand_pool::*;
use crate::utils::{empty_as_none, fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandPoolQueryParams {
 /// "material" | "detail" (default "material")
 #[serde(default, deserialize_with = "empty_as_none")]
 pub view: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub product_id: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_filter: Option<String>,
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_demand_pool_list(
 _path: MesDemandPoolListPath,
 ctx: RequestContext,
 Query(params): Query<DemandPoolQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("WORK_ORDER", "create").await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let svc = state.mes_demand_service();
 let page = params.page.unwrap_or(1);
 let page_params = PageParams::new(page, 20);
 let view_mode = params
 .view
 .as_deref()
 .unwrap_or("material");

 // Parse date_filter into date range
 let (date_start, date_end) = match params.date_filter.as_deref() {
 Some("3days") => {
 let today = chrono::Local::now().date_naive();
 (None, Some(today + chrono::TimeDelta::days(3)))
 }
 Some("7days") => {
 let today = chrono::Local::now().date_naive();
 (None, Some(today + chrono::TimeDelta::days(7)))
 }
 Some("30days") => {
 let today = chrono::Local::now().date_naive();
 (None, Some(today + chrono::TimeDelta::days(30)))
 }
 Some("overdue") => {
 let today = chrono::Local::now().date_naive();
 (None, Some(today))
 }
 _ => (None, None),
 };

 // Fetch both views for stat cards
 let material_result = svc
 .list_material_aggregated(
 &service_ctx,
 &mut conn,
 MaterialAggQuery {
 keyword: params.keyword.clone(),
 required_date_start: date_start,
 required_date_end: date_end,
 ..Default::default()
 },
 PageParams::new(1, 1),
 )
 .await
 .ok();

 let pending_count = material_result.as_ref().map(|r| r.total).unwrap_or(0);

 // Stat: unique material count (from material aggregated)
 let material_count = if view_mode == "material" {
 svc.list_material_aggregated(
 &service_ctx,
 &mut conn,
 MaterialAggQuery {
 keyword: params.keyword.clone(),
 required_date_start: date_start,
 required_date_end: date_end,
 ..Default::default()
 },
 PageParams::new(1, 200),
 )
 .await
 .map(|r| r.items.len() as u64)
 .unwrap_or(0)
 } else {
 0
 };

 // Stat: demands with status InProgress (已创建生产计划)
 let planned_count = svc
 .list_pending_demands(
 &service_ctx,
 &mut conn,
 DemandPoolQuery {
 status: Some(3), // InProgress
 keyword: params.keyword.clone(),
 required_date_start: date_start,
 required_date_end: date_end,
 ..Default::default()
 },
 PageParams::new(1, 1),
 )
 .await
 .map(|r| r.total)
 .unwrap_or(0);

 // Stat: demands due within 3 days (pending only)
 let due_soon_count = svc
 .list_pending_demands(
 &service_ctx,
 &mut conn,
 DemandPoolQuery {
 status: Some(1), // Pending
 keyword: params.keyword.clone(),
 required_date_start: date_start,
 required_date_end: date_end,
 ..Default::default()
 },
 PageParams::new(1, 200),
 )
 .await
 .map(|r| {
 let today = chrono::Local::now().date_naive();
 let deadline = today + chrono::Duration::days(3);
 r.items
 .iter()
 .filter(|d| {
 d.required_date
 .map(|rd| rd <= deadline)
 .unwrap_or(false)
 })
 .count() as u64
 })
 .unwrap_or(0);

 let stats = DemandPoolStats {
 pending_count,
 material_count,
 planned_count,
 due_soon_count,
 };

 // Main content based on view mode
 let view_data = if view_mode == "detail" {
 let query = DemandPoolQuery {
 status: params
 .status
 .as_deref()
 .and_then(|s| s.parse::<i16>().ok()),
 product_id: params
 .product_id
 .as_deref()
 .and_then(|s| s.parse::<i64>().ok()),
 order_id: None,
 keyword: params.keyword.clone(),
 required_date_start: date_start,
 required_date_end: date_end,
 };
 let result = svc
 .list_pending_demands(&service_ctx, &mut conn, query, page_params)
 .await?;
 ViewData::Detail { result }
 } else {
 let result = svc
 .list_material_aggregated(
 &service_ctx,
 &mut conn,
 MaterialAggQuery {
 product_id: params
 .product_id
 .as_deref()
 .and_then(|s| s.parse::<i64>().ok()),
 keyword: params.keyword.clone(),
 required_date_start: date_start,
 required_date_end: date_end,
 },
 page_params,
 )
 .await?;
 ViewData::Material { result }
 };

 let content = demand_pool_page(&stats, &view_data, &params, can_create);

 Ok(Html(
 admin_page(
 is_htmx,
 "生产需求池",
 &claims,
 "production",
 MesDemandPoolListPath::PATH,
 "生产管理",
 Some("生产需求池"),
 content,
 &nav_filter,
 )
 .into_string(),
 ))
}

/// HTMX endpoint: load demand detail rows for a specific product (material expansion)
#[require_permission("WORK_ORDER", "read")]
pub async fn get_demand_rows(
 _path: MesDemandRowsPath,
 ctx: RequestContext,
 Query(params): Query<DemandRowsQueryParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let svc = state.mes_demand_service();
 let query = DemandPoolQuery {
 status: None,
 product_id: Some(params.product_id),
 order_id: None,
 ..Default::default()
 };
 let result = svc
 .list_pending_demands(
 &service_ctx,
 &mut conn,
 query,
 PageParams::new(1, 100),
 )
 .await?;

 Ok(Html(demand_expand_rows(&result.items).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct DemandRowsQueryParams {
 pub product_id: i64,
}

// ── Data holders ──

struct DemandPoolStats {
 pending_count: u64,
 material_count: u64,
 planned_count: u64,
 due_soon_count: u64,
}

enum ViewData {
 Material {
 result: abt_core::shared::types::PaginatedResult<MaterialAggSummary>,
 },
 Detail {
 result: abt_core::shared::types::PaginatedResult<DemandSummary>,
 },
}

// ── Page rendering ──

fn demand_pool_page(
 stats: &DemandPoolStats,
 data: &ViewData,
 params: &DemandPoolQueryParams,
 can_create: bool,
) -> Markup {
 let view_mode = match data {
 ViewData::Material { .. } => "material",
 ViewData::Detail { .. } => "detail",
 };
 let _can_create = can_create;

 html! {
 div {
 // Page header — only refresh button
 div class="flex items-center justify-between mb-6" {
 div {
 h1 class="text-xl font-bold text-fg tracking-tight" { "生产需求池" }
 p class="text-muted text-sm mt-1" {
 "销售订单确认后产生的自制需求，按物料聚合展示。可选择需求创建生产计划草稿。"
 }
 }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-get=(MesDemandPoolListPath::PATH)
 hx-target="#demand-pool-data-card"
 hx-select="#demand-pool-data-card"
 hx-swap="outerHTML" {
 (icon::refresh_icon("w-4 h-4"))
 "刷新"
 }
 }
 }

 // Stat mini cards
 (stat_mini_cards(stats))

 // Data card (includes view toggle + filter so HTMX swap updates active state)
 div id="demand-pool-data-card" {
 (view_toggle_and_filter(view_mode, params))
 @match data {
 ViewData::Material { result } => {
 (material_grid_fragment(result, params))
 }
 ViewData::Detail { result } => {
 (detail_table_fragment(result, params))
 }
 }
 }

 // Batch action bar
 (batch_action_bar())
 }
 }
}

// ── Stat Mini Cards ──

fn stat_mini_cards(stats: &DemandPoolStats) -> Markup {
 html! {
 div class="grid gap-4" {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#fef3c7;color:var(--warn);" {
 (icon::tool_icon(""))
 }
 div {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (stats.pending_count) }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "待处理需求" }
 }
 }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#dbeafe;color:var(--accent);" {
 (icon::cube_icon(""))
 }
 div {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (stats.material_count) }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "涉及物料" }
 }
 }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#dcfce7;color:var(--success);" {
 (icon::check_circle_icon(""))
 }
 div {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (stats.planned_count) }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "计划中" }
 }
 }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#fee2e2;color:var(--danger);" {
 (icon::clock_icon(""))
 }
 div {
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value text-danger" { (stats.due_soon_count) }
 div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "近3日到期" }
 }
 }
 }
 }
}

// ── View Toggle + Filter Bar (same line) ──

fn view_toggle_and_filter(view_mode: &str, params: &DemandPoolQueryParams) -> Markup {
 let is_material = view_mode == "material";
 let material_cls = if is_material { "view-toggle-btn active" } else { "view-toggle-btn" };
 let detail_cls = if is_material { "view-toggle-btn" } else { "view-toggle-btn active" };
 let kw = params.keyword.as_deref().unwrap_or("");
 let df = params.date_filter.as_deref().unwrap_or("");

 html! {
 div class="flex items-center justify-between flex-wrap gap-3" {
 // Left: view toggle
 div class="flex [border-bottom:1px_solid_var(--border-soft)]" {
 button class=(material_cls)
 type="button"
 hx-get=(MesDemandPoolListPath::PATH)
 hx-vals="{\"view\":\"material\"}"
 hx-target="#demand-pool-data-card"
 hx-select="#demand-pool-data-card"
 hx-swap="outerHTML"
 hx-push-url="true"
 hx-include="#mes-filter-form" {
 (icon::grid_4_icon("w-4 h-4"))
 "物料汇总"
 }
 button class=(detail_cls)
 type="button"
 hx-get=(MesDemandPoolListPath::PATH)
 hx-vals="{\"view\":\"detail\"}"
 hx-target="#demand-pool-data-card"
 hx-select="#demand-pool-data-card"
 hx-swap="outerHTML"
 hx-push-url="true"
 hx-include="#mes-filter-form" {
 (icon::rows_icon("w-4 h-4"))
 "订单行明细"
 }
 }

 // Right: search + date filter
 form class="flex items-center gap-3 mb-5 flex-wrap"
 hx-get=(MesDemandPoolListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#demand-pool-data-card"
 hx-select="#demand-pool-data-card"
 hx-swap="outerHTML"
 hx-push-url="true" {
 input type="hidden" name="view" value=(view_mode);
 div class="relative flex-1 max-w-xs" {
 (icon::search_icon("w-4 h-4"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
 placeholder="搜索物料名称、编码…"
 value=(kw);
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="date_filter" {
 option value="" selected[df.is_empty()] { "全部需求日期" }
 option value="3days" selected[df == "3days"] { "近3天到期" }
 option value="7days" selected[df == "7days"] { "近7天到期" }
 option value="30days" selected[df == "30days"] { "近30天到期" }
 option value="overdue" selected[df == "overdue"] { "已逾期" }
 }
 }

 // Hidden form for view toggle to preserve keyword/date_filter
 form id="mes-filter-form" style="display:none;" {
 input type="hidden" name="keyword" value=(kw);
 input type="hidden" name="date_filter" value=(df);
 }
 }
 }
}

// ── Material Grid View (card layout) ──

fn material_grid_fragment(
 result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>,
 params: &DemandPoolQueryParams,
) -> Markup {
 let qs = material_query_string(params.keyword.as_deref(), params.date_filter.as_deref());

 html! {
 div class="data-card" {
 // Column header
 div class="grid items-center gap-6 p-4 border-b-header" {
 div style="flex:1" { "物料信息" }
 div style="width:100px;text-align:center" { "总需求量" }
 div style="width:80px;text-align:center" { "涉及订单" }
 div style="width:160px;text-align:center" { "需求日期范围" }
 div style="width:120px;text-align:center" { "操作" }
 }

 // Material rows
 @if result.items.is_empty() {
 div class="text-center p-6 text-muted text-sm-text" { "暂无待处理需求" }
 }
 @for item in &result.items {
 (material_row(item))
 }

 (pagination(
 MesDemandPoolListPath::PATH,
 &qs,
 result.total,
 result.page,
 result.total_pages,
 ))
 }
 }
}

fn material_row(item: &MaterialAggSummary) -> Markup {
 let pid = item.product_id;
 let hint = urgency_hint(item.earliest_required_date);

 // Date range: "MM/DD → MM/DD"
 let earliest_str = item.earliest_required_date
 .map(|d| d.format("%m/%d").to_string())
 .unwrap_or_else(|| "—".into());
 let latest_str = item.latest_required_date
 .map(|d| d.format("%m/%d").to_string())
 .unwrap_or_else(|| "—".into());
 let date_range = format!("{earliest_str} → {latest_str}");

 // Quantity color class based on urgency
 let qty_cls = demand_qty_class(item.total_demand_qty, item.earliest_required_date);

 // Material icon (color varies by product_id hash)
 let (icon_bg, icon_color, mat_icon) = material_icon(pid);

 html! {
 div class="grid items-center gap-6 p-4 [border-bottom:1px_solid_var(--border-soft)]" {
 // Material info (click toggles .open on expand panel; HTMX loads rows once)
 div class="flex items-center gap-4 cursor-pointer"
 hx-get=(format!("/admin/mes/demand-pool/demand-rows?product_id={pid}"))
 hx-target=(format!("#expand-tbody-{pid}"))
 hx-swap="innerHTML"
 hx-trigger="click once"
 _=(format!("on click toggle .open on #expand-mat-{pid}")) {
 div class="w-[40px] h-[40px] rounded grid place-items-center shrink-0" style=(format!("background:{};color:{}", icon_bg, icon_color)) {
 (mat_icon)
 }
 div {
 div class="font-semibold text-fg text-sm" { (item.product_name) }
 div class="text-[12px] text-muted" { (item.product_code) }
 }
 }

 // Total demand qty
 div class="material-stat" {
 div class=(format!("material-stat-value {qty_cls}")) { (fmt_qty(item.total_demand_qty)) }
 div class="material-text-sm text-muted mt-1" { "总需求量" }
 }

 // Demand count
 div class="material-stat" {
 div class="material-text-2xl font-bold font-mono tabular-nums tabular-nums text-fg accent" { (item.demand_count) }
 div class="material-text-sm text-muted mt-1" { "涉及订单" }
 }

 // Date range
 div class="material-stat material-stat-date" {
 div class="text-[13px] font-semibold text-fg" { (date_range) }
 @if let Some((hint_text, cls)) = &hint {
 div class=(format!("urgency-hint {cls}")) { (hint_text) }
 }
 }

 // Actions (visible on hover)
 div class="flex gap-2 opacity-0" {
 a class="btn inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4"
 href=(format!("{}?product_id={}", MesDemandPoolCreatePath::PATH, pid))
 onclick="event.stopPropagation()" {
 "创建生产计划"
 }
 }
 }

 // Expandable demand detail
 div class="hidden bg-surface-raised [border-bottom:1px_solid_var(--border-soft)]" id=(format!("expand-mat-{pid}")) {
 div class="hidden bg-surface-raised border-b-inner" {
 table class="data-table" {
 thead { tr {
 th style="width:40px" {
 input type="checkbox" title="全选" checked onchange="var cb=this;cb.closest('table').querySelectorAll('input.demand-cb:not([disabled])').forEach(function(c){c.checked=cb.checked;c.dispatchEvent(new Event('change',{bubbles:true}))})";
 }
 th { "需求ID" }
 th { "来源订单" }
 th class="text-right text-[13px]" { "需求数量" }
 th { "需求日期" }
 th { "优先级" }
 th { "状态" }
 }}
 tbody id=(format!("expand-tbody-{pid}")) {
 tr {
 td colspan="7" class="flex items-center justify-center p-8 text-muted" { "加载中..." }
 }
 }
 }
 }
 }
 }
}

// ── Demand Expand Rows (HTMX fragment) ──

fn demand_expand_rows(items: &[DemandSummary]) -> Markup {
 html! {
 @if items.is_empty() {
 tr {
 td colspan="7" class="text-center text-muted" {
 "暂无需求记录"
 }
 }
 }
 @for d in items {
 (demand_expand_row(d))
 }
 }
}

fn demand_expand_row(d: &DemandSummary) -> Markup {
 html! {
 tr class="bg-[rgba(37,99,235,0.04)]" {
 td {
 div class="flex items-center justify-center" {
 input type="checkbox" class="demand-cb" value=(d.id) checked;
 }
 }
 td class="font-mono tabular-nums" style="font-size:12px;" { (d.id) }
 td {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/orders/{}", d.order_id)) style="font-size:12px;" { (d.order_no.as_deref().unwrap_or("—")) }
 }
 td class="text-right text-[13px] font-mono tabular-nums" { (fmt_qty(d.quantity)) }
 td class="font-mono tabular-nums" { (format_date(d.required_date)) }
 td { (priority_label(d.priority)) }
 td { (demand_status_label(d.demand_status)) }
 }
 }
}

// ── Detail View (data-table) ──

fn detail_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<DemandSummary>,
 params: &DemandPoolQueryParams,
) -> Markup {
 let qs = detail_query_string(
 params.keyword.as_deref(),
 params.date_filter.as_deref(),
 params.status.as_deref(),
 params.product_id.as_deref(),
 );

 html! {
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead { tr {
 th style="width:40px" {
 input type="checkbox" title="全选" onchange="var cb=this;cb.closest('table').querySelectorAll('input.demand-cb:not([disabled])').forEach(function(c){c.checked=cb.checked;c.dispatchEvent(new Event('change',{bubbles:true}))})";
 }
 th { "需求ID" }
 th { "产品编码" }
 th { "产品名称" }
 th { "来源订单" }
 th class="text-right text-[13px]" { "需求数量" }
 th { "需求日期" }
 th { "优先级" }
 th { "状态" }
 th { "关联单据" }
 th class="!text-right" { "操作" }
 }}
 tbody {
 @if result.items.is_empty() {
 tr { td colspan="11" class="text-center text-muted" {
 "暂无需求记录"
 }}
 }
 @for item in &result.items {
 (detail_row(item))
 }
 }
 }
 }
 (pagination(
 MesDemandPoolListPath::PATH,
 &qs,
 result.total,
 result.page,
 result.total_pages,
 ))
 }
 }
}

fn detail_row(item: &DemandSummary) -> Markup {
 let is_pending = item.demand_status == 1;
 let row_cls = if is_pending { "" } else { "demand-processed" };

 html! {
 tr class=(row_cls) {
 td {
 @if is_pending {
 input type="checkbox" class="demand-cb" value=(item.id);
 } @else {
 input type="checkbox" class="demand-cb" disabled;
 }
 }
 td class="font-mono tabular-nums" { (item.id) }
 td class="font-mono tabular-nums" { (item.product_code) }
 td { (item.product_name) }
 td {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/orders/{}", item.order_id)) { (item.order_no.as_deref().unwrap_or("—")) }
 }
 td class="text-right text-[13px] font-mono tabular-nums" { (fmt_qty(item.quantity)) }
 td { (format_date(item.required_date)) }
 td { (priority_label(item.priority)) }
 td { (demand_status_label(item.demand_status)) }
 td class="font-mono tabular-nums" {
 @if let (Some(doc_type), Some(doc_id)) = (item.target_doc_type, item.target_doc_id) {
 @if doc_type == 12 {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/plans/{}", doc_id)) { "PP-" (doc_id) }
 } @else if doc_type == 10 {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/orders/{}", doc_id)) { "WO-" (doc_id) }
 } @else {
 "—"
 }
 } @else {
 "—"
 }
 }
 td {
 @if is_pending {
 form method="get" action=(MesDemandPoolCreatePath::PATH) {
 input type="hidden" name="product_id" value=(item.product_id) {}
 button type="submit" class="btn inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" { "创建" }
 }
 } @else {
 @if let (Some(doc_type), Some(doc_id)) = (item.target_doc_type, item.target_doc_id) {
 @if doc_type == 12 {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/plans/{}", doc_id)) { "查看" }
 } @else if doc_type == 10 {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/orders/{}", doc_id)) { "查看" }
 } @else {
 span class="text-muted" { "—" }
 }
 } @else {
 span class="text-muted" { "—" }
 }
 }
 }
 }
 }
}

// ── Batch Action Bar ──

fn batch_action_bar() -> Markup {
 html! {
 div class="hidden fixed bg-[var(--fg)] text-[#fff] rounded-lg p-3 z-[100] items-center gap-5 text-sm opacity-0" id="batchBar" {
 span { "已选择 " span class="batch-count" id="batchCount" { "0" } " 条需求" }
 button class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" id="batchCreateBtn"
 onclick=(format!("window.location.href='{}'", MesDemandPoolCreatePath::PATH)) {
 "创建生产计划"
 }
 button class="btn btn-sm inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" onclick="document.querySelectorAll('input[type=checkbox].demand-cb').forEach(function(c){if(!c.disabled){c.checked=false}});document.getElementById('batchBar').classList.remove('show')" { "清除选择" }
 }

 (PreEscaped(r#"<script>
 document.addEventListener('change',function(e){
 if(e.target.type==='checkbox'&&e.target.classList.contains('demand-cb')){
 var tr=e.target.closest('tr');
 if(tr){
 if(e.target.checked){tr.classList.add('demand-row-selected');}
 else{tr.classList.remove('demand-row-selected');}
 }
 updateBatchBar();
 }
 });
 function updateBatchBar(){
 var checked=document.querySelectorAll('input[type=checkbox].demand-cb:checked:not([disabled])');
 var count=checked.length;
 var bar=document.getElementById('batchBar');
 if(count>0){
 var ids=[];
 checked.forEach(function(c){ids.push(c.value);});
 bar.classList.add('show');
 document.getElementById('batchCount').textContent=count;
 document.getElementById('batchCreateBtn').href='/admin/mes/demand-pool/create?demand_ids='+ids.join(',');
 }else{
 bar.classList.remove('show');
 }
 }
 </script>"#))
 }
}

// ── Helpers ──

/// Returns (icon_bg_color, icon_text_color, icon_markup) based on product_id hash
fn material_icon(product_id: i64) -> (String, String, Markup) {
 let variant = (product_id % 4) as u8;
 match variant {
 0 => (
 "#fef3c7".into(),
 "var(--warn)".into(),
 icon::tool_icon(""),
 ),
 1 => (
 "#ede9fe".into(),
 "#7c3aed".into(),
 icon::cube_icon(""),
 ),
 2 => (
 "#dbeafe".into(),
 "var(--accent)".into(),
 icon::briefcase_icon(""),
 ),
 _ => (
 "#dcfce7".into(),
 "var(--success)".into(),
 icon::check_circle_icon(""),
 ),
 }
}

/// Determine quantity display color class based on total qty and earliest date
fn demand_qty_class(total: rust_decimal::Decimal, earliest: Option<NaiveDate>) -> String {
 // Check urgency first
 if let Some(d) = earliest {
 let today = chrono::Local::now().date_naive();
 let diff = (d - today).num_days();
 if diff <= 3 {
 return "danger".into();
 }
 if diff <= 7 {
 return "warn".into();
 }
 }
 // Check magnitude
 if total > rust_decimal::Decimal::from(100) {
 return "warn".into();
 }
 "accent".into()
}

/// Urgency hint text and CSS class for earliest required date
fn urgency_hint(earliest: Option<NaiveDate>) -> Option<(String, &'static str)> {
 earliest.and_then(|d| {
 let today = chrono::Local::now().date_naive();
 let diff = (d - today).num_days();
 if diff < 0 {
 Some((format!("⚠ 已逾期{}天", diff.abs()), "text-danger"))
 } else if diff == 0 {
 Some(("⚠ 今天到期".into(), "text-danger"))
 } else if diff <= 3 {
 Some((format!("⚠ {}天后到期", diff), "text-danger"))
 } else if diff <= 7 {
 Some((format!("{}天后到期", diff), "text-warn"))
 } else if diff <= 30 {
 Some((format!("{}天后到期", diff), "text-muted"))
 } else {
 None
 }
 })
}

fn material_query_string(keyword: Option<&str>, date_filter: Option<&str>) -> String {
 let mut q = vec![];
 if let Some(kw) = keyword
 && !kw.is_empty()
 {
 q.push(format!("keyword={kw}"));
 }
 if let Some(df) = date_filter
 && !df.is_empty()
 {
 q.push(format!("date_filter={df}"));
 }
 q.join("&")
}

fn detail_query_string(
 keyword: Option<&str>,
 date_filter: Option<&str>,
 status: Option<&str>,
 product_id: Option<&str>,
) -> String {
 let mut q = vec!["view=detail".to_string()];
 if let Some(kw) = keyword
 && !kw.is_empty()
 {
 q.push(format!("keyword={kw}"));
 }
 if let Some(df) = date_filter
 && !df.is_empty()
 {
 q.push(format!("date_filter={df}"));
 }
 if let Some(s) = status
 && !s.is_empty()
 {
 q.push(format!("status={s}"));
 }
 if let Some(pid) = product_id
 && !pid.is_empty()
 {
 q.push(format!("product_id={pid}"));
 }
 q.join("&")
}

fn format_date(d: Option<NaiveDate>) -> Markup {
 match d {
 Some(date) => html! { (date.format("%Y-%m-%d").to_string()) },
 None => html! { span class="text-muted" { "—" } },
 }
}

fn demand_status_label(status: i16) -> Markup {
 let (label, cls) = match status {
 1 => ("待处理", "status-pill-muted"),
 2 => ("已确认", "status-pill-info"),
 3 => ("已创建生产计划", "status-pill-warn"),
 4 => ("已完成", "status-pill-success"),
 5 => ("已拒绝", "status-pill-danger"),
 _ => ("未知", "status-pill-muted"),
 };
 html! {
 span class=(format!("status-pill {cls}")) { (label) }
 }
}

fn priority_label(priority: i32) -> Markup {
 let (label, cls) = match priority {
 p if p >= 4 => ("紧急", "tag-danger"),
 3 => ("高", "tag-warn"),
 2 => ("中", "tag-info"),
 _ => ("低", "tag-muted"),
 };
 html! {
 span class=(format!("tag-chip {cls}")) { (label) }
 }
}

