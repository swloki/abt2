use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::inventory_lock::model::*;
use abt_core::wms::inventory_lock::InventoryLockService;
use abt_core::wms::enums::LockStatus;
use abt_core::master_data::product::ProductService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;
use abt_core::master_data::customer::CustomerService;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_inventory_lock::{
 LockCreatePath, LockDetailPath, LockListPath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LockQueryParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub doc_number: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub product: Option<String>,
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_lock_list(
 _path: LockListPath,
 ctx: RequestContext,
 Query(params): Query<LockQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let can_create = ctx.has_permission("INVENTORY", "create").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.inventory_lock_service();

 let filter = build_filter(&params);
 let page_num = params.page.unwrap_or(1);

 let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;

 // batch resolve IDs
 let product_svc = state.product_service();
 let mut product_map: std::collections::HashMap<i64, (String, String)> = std::collections::HashMap::new();
 for lock in &result.items {
 if !product_map.contains_key(&lock.product_id)
 && let Ok(p) = product_svc.get(&service_ctx, &mut conn, lock.product_id).await {
 product_map.insert(lock.product_id, (p.product_code, p.pdt_name));
 }
 }

 let wh_svc = state.warehouse_service();
 let mut wh_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 for lock in &result.items {
 if !wh_names.contains_key(&lock.warehouse_id)
 && let Ok(w) = wh_svc.get(&service_ctx, &mut conn, lock.warehouse_id).await {
 wh_names.insert(lock.warehouse_id, w.name);
 }
 }

 let user_svc = state.user_service();
 let operator_ids: Vec<i64> = result.items.iter().map(|l| l.operator_id).collect();
 let operator_map = user_svc.get_users_by_ids(&service_ctx, &mut conn, operator_ids)
 .await
 .map(|users| users.into_iter().map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username))).collect::<std::collections::HashMap<i64, String>>())
 .unwrap_or_default();

 let customer_svc = state.customer_service();
 let mut customer_map: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 for lock in &result.items {
 if let Some(cid) = lock.customer_id
 && !customer_map.contains_key(&cid)
 && let Ok(c) = customer_svc.get(&service_ctx, &mut conn, cid).await {
 customer_map.insert(cid, c.name);
 }
 }

 let content = lock_list_page(&result, &params, &product_map, &wh_names, &operator_map, &customer_map, can_create);
 let page_html = admin_page(
 is_htmx,
 "库存锁定",
 &claims,
 "inventory",
 LockListPath::PATH,
 "库存管理",
 Some("库存锁定"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &LockQueryParams) -> LockFilter {
 LockFilter {
 status: params.status.and_then(LockStatus::from_i16),
 warehouse_id: params.warehouse_id,
 product_id: None,
 customer_id: None,
 }
}

fn status_label(s: &LockStatus) -> &'static str {
 match s {
 LockStatus::Active => "生效",
 LockStatus::Released => "已释放",
 LockStatus::Cancelled => "已作废",
 }
}

fn status_class(s: &LockStatus) -> &'static str {
 match s {
 LockStatus::Active => "status-progress",
 LockStatus::Released => "status-completed",
 LockStatus::Cancelled => "status-cancelled",
 }
}

// ── Components ──

fn lock_list_page(
 result: &abt_core::shared::types::PaginatedResult<InventoryLock>,
 params: &LockQueryParams,
 product_map: &std::collections::HashMap<i64, (String, String)>,
 wh_names: &std::collections::HashMap<i64, String>,
 operator_map: &std::collections::HashMap<i64, String>,
 customer_map: &std::collections::HashMap<i64, String>,
 can_create: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "库存锁定" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(LockCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建锁库"
 }
 }
 }
 }

 (lock_table_fragment(result, params, product_map, wh_names, operator_map, customer_map))
 }
 }
}

fn lock_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<InventoryLock>,
 params: &LockQueryParams,
 product_map: &std::collections::HashMap<i64, (String, String)>,
 wh_names: &std::collections::HashMap<i64, String>,
 operator_map: &std::collections::HashMap<i64, String>,
 customer_map: &std::collections::HashMap<i64, String>,
) -> Markup {
 let _query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "生效", count: None },
 TabItem { value: "2".into(), label: "已释放", count: None },
 TabItem { value: "3".into(), label: "已作废", count: None },
 ];

 html! {
 div class="lock-list-panel" {
 (status_tabs_with_param(LockListPath::PATH, "#lock-data-card", "#lock-filter-form", tabs, &active_value, "status"))

 form class="flex items-center gap-3 mb-6 flex-wrap" id="lock-filter-form"
 hx-get=(LockListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#lock-data-card"
 hx-select="#lock-data-card"
 hx-swap="outerHTML"
 hx-include="#lock-filter-form"
 hx-push-url="true" {
 div class="relative w-60 [&_[class*=i-lucide]]:absolute [&_[class*=i-lucide]]:left-3 [&_[class*=i-lucide]]:top-1/2 [&_[class*=i-lucide]]:-translate-y-1/2 [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4 [&_[class*=i-lucide]]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="doc_number"
 placeholder="搜索锁库单号…"
 value=(params.doc_number.as_deref().unwrap_or(""));
 }
 div class="relative w-40 [&_[class*=i-lucide]]:absolute [&_[class*=i-lucide]]:left-3 [&_[class*=i-lucide]]:top-1/2 [&_[class*=i-lucide]]:-translate-y-1/2 [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4 [&_[class*=i-lucide]]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="product"
 placeholder="产品编码/名称"
 value=(params.product.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="warehouse_id" {
 option value="" { "全部仓库" }
 }
 }

 (lock_data_card_fragment(result, params, product_map, wh_names, operator_map, customer_map))
 }
 }
}

fn lock_data_card_fragment(
 result: &abt_core::shared::types::PaginatedResult<InventoryLock>,
 params: &LockQueryParams,
 product_map: &std::collections::HashMap<i64, (String, String)>,
 wh_names: &std::collections::HashMap<i64, String>,
 operator_map: &std::collections::HashMap<i64, String>,
 customer_map: &std::collections::HashMap<i64, String>,
) -> Markup {
 let query = build_query_string(params);

 html! {
 div class="data-card" id="lock-data-card" {
 div class="overflow-x-auto" {
 table class="data-table min-w-[1060px]" {
 thead {
 tr {
 th { "锁库单号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "锁定仓库" }
 th class="text-right text-[13px]" { "锁定数量" }
 th { "锁定原因" }
 th { "关联客户" }
 th { "状态" }
 th { "操作员" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for lock in &result.items {
 (lock_row(lock, product_map, wh_names, operator_map, customer_map))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="10" class="text-center text-muted p-8" {
 "暂无锁库数据"
 }
 }
 }
 }
 }
 }
 (pagination(LockListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
}

fn lock_row(
 lock: &InventoryLock,
 product_map: &std::collections::HashMap<i64, (String, String)>,
 wh_names: &std::collections::HashMap<i64, String>,
 operator_map: &std::collections::HashMap<i64, String>,
 customer_map: &std::collections::HashMap<i64, String>,
) -> Markup {
 let detail_path = LockDetailPath { id: lock.id }.to_string();
 let sl = status_label(&lock.status);
 let sc = status_class(&lock.status);
 let locked_qty_fmt = format!("{:.2}", lock.locked_qty);

 html! {
 tr {
 td {
 a class="text-accent font-medium font-mono tabular-nums hover:underline" href=(&detail_path) { (lock.doc_number) }
 }
 td class="font-mono tabular-nums" {
 (product_map.get(&lock.product_id).map(|(c,_)| c.as_str()).unwrap_or("—"))
 }
 td {
 (product_map.get(&lock.product_id).map(|(_,n)| n.as_str()).unwrap_or("—"))
 }
 td {
 (wh_names.get(&lock.warehouse_id).map(|s| s.as_str()).unwrap_or("—"))
 }
 td class="text-right text-[13px] font-mono tabular-nums" { (locked_qty_fmt) }
 td { (lock.lock_reason) }
 td {
 @if let Some(cid) = lock.customer_id {
 (customer_map.get(&cid).map(|s| s.as_str()).unwrap_or("—"))
 } @else {
 span class="text-muted" { "—" }
 }
 }
 td {
 span class=(format!("status-pill {}", crate::utils::status_color(sc))) { (sl) }
 }
 td {
 (operator_map.get(&lock.operator_id).map(|s| s.as_str()).unwrap_or("—"))
 }
 td {
 div class="flex items-center gap-1 justify-end" {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer hover:bg-accent-bg" href=(&detail_path) title="查看" {
 (icon::eye_icon("w-4 h-4"))
 }
 }
 }
 }
}
}

fn build_query_string(params: &LockQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref v) = params.doc_number {
 q.push(format!("doc_number={v}"));
 }
 if let Some(ref v) = params.product {
 q.push(format!("product={v}"));
 }
 if let Some(s) = params.status {
 q.push(format!("status={s}"));
 }
 if let Some(w) = params.warehouse_id {
 q.push(format!("warehouse_id={w}"));
 }
 if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) }
}
