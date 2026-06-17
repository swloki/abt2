use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::enums::BinStatus;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::wms_bin::{BinCreatePath, BinDetailPath, BinListPath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BinQueryParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub code: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub name: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("LOCATION", "read")]
pub async fn get_bin_list(
 _path: BinListPath,
 ctx: RequestContext,
 Query(params): Query<BinQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let can_create = ctx.has_permission("LOCATION", "create").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.warehouse_service();

 let search_params = build_search_params(&params);
 let result = svc.search_bins_with_warehouse(&service_ctx, &mut conn, search_params).await?;
 let warehouses = svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await?;

 // Build zone lookup for display
 let mut zone_map: HashMap<i64, Zone> = HashMap::new();
 let mut fetched_wh: HashSet<i64> = HashSet::new();
 for item in &result.items {
 if fetched_wh.insert(item.warehouse_id)
 && let Ok(zs) = svc.list_zones(&service_ctx, &mut conn, item.warehouse_id).await {
 for z in zs {
 zone_map.insert(z.id, z);
 }
 }
 }

 let content = bin_list_page(&result, &params, &warehouses.items, &zone_map, can_create);
 let page_html = admin_page(
 is_htmx,
 "储位管理",
 &claims,
 "inventory",
 BinListPath::PATH,
 "库存管理",
 Some("储位管理"),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_search_params(params: &BinQueryParams) -> SearchBinsParams {
 let is_active = match params.status {
 Some(4) => Some(false),
 Some(1) | Some(2) | Some(3) => Some(true),
 _ => None,
 };
 SearchBinsParams {
 keyword: match (&params.code, &params.name) {
 (Some(c), Some(n)) if !c.trim().is_empty() && !n.trim().is_empty() => Some(format!("{} {}", c.trim(), n.trim())),
 (Some(c), _) if !c.trim().is_empty() => Some(c.trim().to_string()),
 (_, Some(n)) if !n.trim().is_empty() => Some(n.trim().to_string()),
 _ => None,
 },
 is_active,
 warehouse_id: params.warehouse_id,
 page: params.page.unwrap_or(1),
 page_size: 20,
 }
}

pub(crate) fn bin_status_label(s: &BinStatus) -> &'static str {
 match s {
 BinStatus::Empty => "空闲",
 BinStatus::Occupied => "占用",
 BinStatus::Locked => "锁定",
 BinStatus::Disabled => "停用",
 }
}

pub(crate) fn bin_status_class(s: &BinStatus) -> &'static str {
 match s {
 BinStatus::Empty => "status-completed",
 BinStatus::Occupied => "status-accepted",
 BinStatus::Locked => "status-progress",
 BinStatus::Disabled => "status-draft",
 }
}

// ── Components ──

fn bin_list_page(
 result: &abt_core::shared::types::PaginatedResult<BinWithWarehouse>,
 params: &BinQueryParams,
 warehouses: &[Warehouse],
 zones: &HashMap<i64, Zone>,
 can_create: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "储位管理" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(BinCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建储位"
 }
 }
 }
 }

 (bin_table_fragment(result, params, warehouses, zones))
 }
 }
}

/// The data-card with table + pagination. This is the HTMX swap target.
fn bin_data_card(
 result: &abt_core::shared::types::PaginatedResult<BinWithWarehouse>,
 params: &BinQueryParams,
 zones: &HashMap<i64, Zone>,
) -> Markup {
 let query = build_query_string(params);

 html! {
 div id="bin-data-card" class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "储位编码" }
 th { "储位名称" }
 th { "所属仓库" }
 th { "所属库区" }
 th class="text-right text-[13px]" { "行" }
 th class="text-right text-[13px]" { "列" }
 th class="text-right text-[13px]" { "层" }
 th class="text-right text-[13px]" { "容量上限" }
 th { "当前状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for item in &result.items {
 (bin_row(item, zones))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
 "暂无储位数据"
 }
 }
 }
 }
 }
 }
 (pagination(BinListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
}

fn bin_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<BinWithWarehouse>,
 params: &BinQueryParams,
 warehouses: &[Warehouse],
 zones: &HashMap<i64, Zone>,
) -> Markup {
 let _query = build_query_string(params);

 html! {
 div class="bin-list-panel" {
 // ── Filter Bar ──
 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="filter-form"
 hx-get=(BinListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#bin-data-card"
 hx-select="#bin-data-card"
 hx-swap="outerHTML"
 hx-include="#filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs" {
 (icon::search_icon("w-4 h-4"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code"
 style="width:180px"
 placeholder="储位编码"
 value=(params.code.as_deref().unwrap_or(""));
 }
 div class="relative flex-1 max-w-xs" {
 (icon::search_icon("w-4 h-4"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name"
 placeholder="储位名称"
 value=(params.name.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="warehouse_id" {
 option value="" { "全部仓库" }
 @for wh in warehouses {
 option value=(wh.id) selected[params.warehouse_id == Some(wh.id)] {
 (wh.name)
 }
 }
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="status" {
 option value="" { "全部状态" }
 option value="1" selected[params.status == Some(1)] { "空闲" }
 option value="2" selected[params.status == Some(2)] { "占用" }
 option value="3" selected[params.status == Some(3)] { "锁定" }
 option value="4" selected[params.status == Some(4)] { "停用" }
 }
 }

 // ── Data Table ──
 (bin_data_card(result, params, zones))
 }
 }
}

fn bin_row(item: &BinWithWarehouse, zones: &HashMap<i64, Zone>) -> Markup {
 let detail_path = BinDetailPath { id: item.bin.id }.to_string();
 let bin = &item.bin;
 let status_label = bin_status_label(&bin.status);
 let status_class = bin_status_class(&bin.status);
 let zone_name = zones.get(&bin.zone_id).map(|z| z.name.as_str()).unwrap_or("—");

 html! {
 tr style="cursor:pointer" {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (bin.code) }
 td onclick=(format!("location.href='{}'", detail_path)) { (bin.name) }
 td onclick=(format!("location.href='{}'", detail_path)) { (item.warehouse_name) }
 td onclick=(format!("location.href='{}'", detail_path)) { (zone_name) }
 td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) {
 (bin.row_no.as_deref().unwrap_or("—"))
 }
 td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) {
 (bin.column_no.as_deref().unwrap_or("—"))
 }
 td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) {
 (bin.layer_no.as_deref().unwrap_or("—"))
 }
 td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) {
 @if let Some(cap) = &bin.capacity_limit {
 (format!("{:.2}", cap))
 } @else {
 "—"
 }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class=(format!("status-pill {status_class}")) { (status_label) }
 }
 td onclick="event.stopPropagation()" {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="查看详情" href=(detail_path) {
 (icon::eye_icon("w-4 h-4"))
 }
 }
 }
 }
 }
}

fn build_query_string(params: &BinQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref v) = params.code
 && !v.trim().is_empty() {
 q.push(format!("code={v}"));
 }
 if let Some(ref v) = params.name
 && !v.trim().is_empty() {
 q.push(format!("name={v}"));
 }
 if let Some(w) = params.warehouse_id {
 q.push(format!("warehouse_id={w}"));
 }
 if let Some(s) = params.status {
 q.push(format!("status={s}"));
 }
 q.join("&")
}
