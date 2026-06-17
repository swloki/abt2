use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::enums::{WarehouseStatus, WarehouseType};
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::components::import_modal::{self, ImportModalConfig};
use crate::components::export_button;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_warehouse::{
 WarehouseCreatePath, WarehouseDeletePath, WarehouseDetailPath, WarehouseEditPath,
 WarehouseListPath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WarehouseQueryParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub code: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub name: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_type: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WAREHOUSE", "read")]
pub async fn get_warehouse_list(
 _path: WarehouseListPath,
 ctx: RequestContext,
 Query(params): Query<WarehouseQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let can_create = ctx.has_permission("WAREHOUSE", "create").await;
 let can_delete = ctx.has_permission("WAREHOUSE", "delete").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.warehouse_service();

 let filter = build_filter(&params);
 let page_num = params.page.unwrap_or(1);

 let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

 // 解析管理员名称
 let manager_ids: Vec<i64> = result.items.iter()
 .filter_map(|w| w.manager_id)
 .collect();
 let manager_map = if manager_ids.is_empty() {
 std::collections::HashMap::new()
 } else {
 state.user_service()
 .get_users_by_ids(&service_ctx, &mut conn, manager_ids)
 .await
 .map(|users| {
 users.into_iter()
 .map(|u| {
 let name = u.user.display_name.unwrap_or(u.user.username);
 (u.user.user_id, name)
 })
 .collect::<std::collections::HashMap<i64, String>>()
 })
 .unwrap_or_default()
 };

 let content = warehouse_list_page(&result, &params, &manager_map, can_create, can_delete);
 let page_html = admin_page(
 is_htmx,
 "仓库管理",
 &claims,
 "inventory",
 WarehouseListPath::PATH,
 "库存管理",
 Some("仓库管理"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &WarehouseQueryParams) -> WarehouseFilter {
 let keyword = match (&params.code, &params.name) {
 (Some(c), Some(n)) if !c.is_empty() && !n.is_empty() => Some(format!("{} {}", c, n)),
 (Some(c), _) if !c.is_empty() => Some(c.clone()),
 (_, Some(n)) if !n.is_empty() => Some(n.clone()),
 _ => None,
 };
 WarehouseFilter {
 warehouse_type: params.warehouse_type.and_then(WarehouseType::from_i16),
 status: params.status.and_then(WarehouseStatus::from_i16),
 keyword,
 }
}

fn warehouse_type_label(t: &WarehouseType) -> &'static str {
 match t {
 WarehouseType::RawMaterial => "原材料仓",
 WarehouseType::FinishedGoods => "成品仓",
 WarehouseType::SemiFinished => "半成品仓",
 WarehouseType::Consumable => "辅料仓",
 WarehouseType::VirtualOutsource => "虚拟仓",
 }
}

fn warehouse_status_label(s: &WarehouseStatus) -> &'static str {
 match s {
 WarehouseStatus::Active => "启用",
 WarehouseStatus::Inactive => "停用",
 }
}

fn warehouse_status_class(s: &WarehouseStatus) -> &'static str {
 match s {
 WarehouseStatus::Active => "status-accepted",
 WarehouseStatus::Inactive => "status-rejected",
 }
}

// ── Components ──

fn warehouse_list_page(
 result: &abt_core::shared::types::PaginatedResult<Warehouse>,
 params: &WarehouseQueryParams,
 manager_map: &std::collections::HashMap<i64, String>,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "仓库管理" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _=(import_modal::import_modal_onclick(&ImportModalConfig { import_type: "warehouse-location", title: "", template_columns: "" })) {
 (icon::upload_icon("w-4 h-4"))
 "导入"
 }
 (export_button::export_button("导出库位", "warehouse-location"))
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(WarehouseCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建仓库"
 }
 }
 }
 }

 // ── Tabs + Filter + Data Table (HTMX panel) ──
 (warehouse_table_fragment(result, params, manager_map, can_delete))

 // ── Import Modal ──
 (import_modal::import_modal(&ImportModalConfig {
 import_type: "warehouse-location",
 title: "导入仓库库位",
 template_columns: "仓库编码, 仓库名称, 库位编码, 库位名称, 容量",
 }))

 }
 }
}

fn warehouse_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<Warehouse>,
 params: &WarehouseQueryParams,
 manager_map: &std::collections::HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "启用", count: None },
 TabItem { value: "2".into(), label: "停用", count: None },
 ];

 html! {
 div class="warehouse-list-panel" {
 (status_tabs_with_param(WarehouseListPath::PATH, "#warehouse-data-card", "#warehouse-filter-form", tabs, &active_value, "status"))

 // ── Filter Bar ──
 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="warehouse-filter-form"
 hx-get=(WarehouseListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#warehouse-data-card"
 hx-select="#warehouse-data-card"
 hx-swap="outerHTML"
 hx-include="#warehouse-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code"
 style="width:180px"
 placeholder="仓库编码"
 value=(params.code.as_deref().unwrap_or(""));
 }
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name"
 placeholder="仓库名称"
 value=(params.name.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="warehouse_type" {
 option value="" { "全部类型" }
 option value="1" selected[params.warehouse_type == Some(1)] { "原材料仓" }
 option value="2" selected[params.warehouse_type == Some(2)] { "成品仓" }
 option value="3" selected[params.warehouse_type == Some(3)] { "半成品仓" }
 option value="4" selected[params.warehouse_type == Some(4)] { "辅料仓" }
 option value="5" selected[params.warehouse_type == Some(5)] { "虚拟仓" }
 }
 }

 (warehouse_data_card(result, &query, manager_map, can_delete))
 }
 }
}

fn warehouse_data_card(
 result: &abt_core::shared::types::PaginatedResult<Warehouse>,
 query: &str,
 manager_map: &std::collections::HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 html! {
 div id="warehouse-data-card" class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "仓库编码" }
 th { "仓库名称" }
 th { "仓库类型" }
 th { "状态" }
 th { "地址" }
 th { "管理员" }
 th { "库区数" }
 th { "储位数" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for w in &result.items {
 (warehouse_row(w, manager_map, can_delete))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
 "暂无仓库数据"
 }
 }
 }
 }
 }
 }
 (pagination(WarehouseListPath::PATH, query, result.total, result.page, result.total_pages))
 }
 }
}

fn warehouse_row(w: &Warehouse, manager_map: &std::collections::HashMap<i64, String>, can_delete: bool) -> Markup {
 let detail_path = WarehouseDetailPath { id: w.id }.to_string();
 let edit_path = WarehouseEditPath { id: w.id }.to_string();
 let delete_path = WarehouseDeletePath { id: w.id };

 let type_label = warehouse_type_label(&w.warehouse_type);
 let status_label = warehouse_status_label(&w.status);
 let status_class = warehouse_status_class(&w.status);

 html! {
 tr style="cursor:pointer" {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (w.code) }
 td onclick=(format!("location.href='{}'", detail_path)) { strong { (w.name) } }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-surface text-[#666]" { (type_label) }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class=(format!("status-pill {status_class}")) { (status_label) }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 @if w.is_virtual {
 span style="color:var(--muted)" { "—" }
 } @else if let Some(ref addr) = w.address {
 (addr)
 } @else {
 span style="color:var(--muted)" { "—" }
 }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 @if let Some(mid) = w.manager_id {
 @if let Some(name) = manager_map.get(&mid) {
 (name)
 } @else {
 span style="color:var(--muted)" { "—" }
 }
 } @else {
 span style="color:var(--muted)" { "—" }
 }
 }
 td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) {
 @if w.zone_count > 0 {
 (w.zone_count)
 } @else {
 span style="color:var(--muted)" { "0" }
 }
 }
 td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) {
 @if w.bin_count > 0 {
 (w.bin_count)
 } @else {
 span style="color:var(--muted)" { "0" }
 }
 }
 td onclick="event.stopPropagation()" {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑" href=(edit_path) {
 (icon::edit_icon("w-4 h-4"))
 }
 @if can_delete {
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
 hx-post=(delete_path)
 hx-confirm=(format!("删除后无法恢复，确定要删除仓库 <strong>{}</strong> 吗？", w.name))
 hx-target="closest tr"
 hx-swap="outerHTML swap:0.5s" {
 (icon::trash_icon("w-4 h-4"))
 }
 }
 }
 }
 }
 }
}

fn build_query_string(params: &WarehouseQueryParams) -> String {
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
 if let Some(t) = params.warehouse_type {
 q.push(format!("warehouse_type={t}"));
 }
 q.join("&")
}
