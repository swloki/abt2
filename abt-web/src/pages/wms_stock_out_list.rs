use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::UserService;
use abt_core::wms::enums::TransactionType;
use abt_core::wms::inventory_transaction::model::TransactionFilter;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::warehouse::{WarehouseFilter, WarehouseService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::{StockOutCreatePath, StockOutListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct StockOutQueryParams {
 pub doc_number: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub product_code: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub transaction_type: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_start: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_end: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn out_type_label(t: &TransactionType) -> (&'static str, &'static str) {
 // (label, pill_class)
 match t {
 TransactionType::SalesShipment => ("销售出库", "bg-[#fff1f0] text-danger"),
 TransactionType::MaterialIssue => ("生产领料", "bg-[#fff7e6] text-warn"),
 _ => ("其他", "bg-surface text-muted"),
 }
}

async fn resolve_operator_names<S: UserService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
) -> HashMap<i64, String> {
 let unique_ids: Vec<i64> = items.iter().map(|t| t.operator_id).collect();
 if unique_ids.is_empty() {
 return HashMap::new();
 }
 let mut map = HashMap::new();
 for id in unique_ids {
 if !map.contains_key(&id)
 && let Ok(user) = svc.get_user(ctx, db, id).await {
 map.insert(id, user.display_name.unwrap_or_default());
 }
 }
 map
}

async fn resolve_wh_names<S: WarehouseService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
) -> HashMap<i64, String> {
 let unique_ids: Vec<i64> = items.iter().map(|t| t.warehouse_id).collect();
 if unique_ids.is_empty() {
 return HashMap::new();
 }
 let mut map = HashMap::new();
 for id in unique_ids {
 if !map.contains_key(&id)
 && let Ok(wh) = svc.get(ctx, db, id).await {
 map.insert(id, wh.name);
 }
 }
 map
}

fn build_query_string(params: &StockOutQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref v) = params.doc_number {
 q.push(format!("doc_number={v}"));
 }
 if let Some(ref v) = params.product_code {
 q.push(format!("product_code={v}"));
 }
 if let Some(ref tt) = params.transaction_type {
 q.push(format!("transaction_type={tt}"));
 }
 if let Some(wid) = params.warehouse_id {
 q.push(format!("warehouse_id={wid}"));
 }
 if let Some(ref ds) = params.date_start {
 q.push(format!("date_start={ds}"));
 }
 if let Some(ref de) = params.date_end {
 q.push(format!("date_end={de}"));
 }
 q.join("&")
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_stock_out_list(
 _path: StockOutListPath,
 ctx: RequestContext,
 Query(params): Query<StockOutQueryParams>,
) -> Result<Html<String>> {
 let can_create = ctx.has_permission("INVENTORY", "create").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.inventory_transaction_service();
 let user_svc = state.user_service();
 let warehouse_svc = state.warehouse_service();

 let txn_type = params.transaction_type.as_deref().and_then(TransactionType::from_name);
 let filter = TransactionFilter {
 transaction_type: txn_type,
 product_id: None,
 warehouse_id: params.warehouse_id,
 source_type: None,
 source_id: None,
 doc_number: params.doc_number.clone(),
 product_code: params.product_code.clone(),
 };
 let page_num = params.page.unwrap_or(1);
 let result = svc.query(&service_ctx, &mut conn, filter, page_num, 20).await?;

 let operator_names = resolve_operator_names(&user_svc, &service_ctx, &mut conn, &result.items).await;
 let wh_names = resolve_wh_names(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
 let warehouses = warehouse_svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await.map(|r| r.items).unwrap_or_default();

 let content = stock_out_list_page(&result, &operator_names, &wh_names, &warehouses, &params, can_create);
 let page_html = admin_page(
 is_htmx, "出库管理", &claims, "inventory", StockOutListPath::PATH, "库存管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn stat_card(icon_markup: &Markup, icon_cls: &str, value: &str, label: &str) -> Markup {
 html! {
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md shadow-xs" {
 div class=(format!("w-[44px] h-[44px] rounded-md grid place-items-center shrink-0 {}", icon_cls)) {
 (icon_markup)
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { (value) }
 div class="text-sm text-muted mt-1" { (label) }
 }
 }
 }
}

fn stock_out_list_page(
 result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
 operator_names: &HashMap<i64, String>,
 wh_names: &HashMap<i64, String>,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 params: &StockOutQueryParams,
 can_create: bool,
) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "出库管理" }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::download_icon("w-4 h-4"))
 "导出"
 }
 @if can_create {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(StockOutCreatePath::PATH) {
 (icon::upload_icon("w-4 h-4"))
 "新建出库单"
 }
 }
 }
 }

 // ── Tabs + Filter + Table (HTMX panel) ──
 (stock_out_table_fragment(result, operator_names, wh_names, warehouses, params))
 }
 }
}

fn stock_out_data_card(
 result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
 operator_names: &HashMap<i64, String>,
 wh_names: &HashMap<i64, String>,
 query: &str,
) -> Markup {
 html! {
 div class="data-card" id="stockout-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-[30px]" { input type="checkbox" class="cursor-pointer"; }
 th { "出库单号" }
 th { "出库类型" }
 th { "来源单号" }
 th { "来源仓库" }
 th { "物料数量" }
 th class="text-right text-[13px]" { "出库总量" }
 th class="text-right text-[13px]" { "总金额" }
 th { "拣货策略" }
 th { "状态" }
 th { "操作员" }
 th { "出库时间" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for item in &result.items {
 @let (type_label, type_cls) = out_type_label(&item.transaction_type);
 @let wh_name = wh_names.get(&item.warehouse_id).map(|s| s.as_str()).unwrap_or("—");
 @let op_name = operator_names.get(&item.operator_id).map(|s| s.as_str()).unwrap_or("—");
 @let detail_url = format!("{}/{}", StockOutListPath::PATH, item.id);
 tr {
 td { input type="checkbox" class="cursor-pointer"; }
 td {
 a class="font-mono tabular-nums text-accent hover:underline" href=(detail_url) { (item.doc_number.as_deref().unwrap_or("—")) }
 }
 td {
 span class=(format!("inline-flex items-center gap-1 rounded-full text-xs font-medium px-2 py-0.5 {}", type_cls)) {
 (type_label)
 }
 }
 td class="font-mono tabular-nums text-xs text-fg-2" { (format!("{}-{}", item.source_type, item.source_id)) }
 td class="text-sm text-fg" { (wh_name) }
 td class="text-sm text-muted" { "1 种" }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.quantity)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (item.unit_cost.map(|c| format!("¥{:.2}", c)).unwrap_or_else(|| "—".into())) }
 td class="text-xs text-fg-2" { "FIFO" }
 td {
 span class="status-pill bg-[#f0fff0] text-[#389e0d]" { "已出库" }
 }
 td class="text-sm text-fg" { (op_name) }
 td class="text-xs text-muted" { (item.created_at.format("%Y-%m-%d %H:%M")) }
 td {
 a href=(detail_url) class="text-accent text-xs hover:underline" { "详情" }
 }
 }
 }
 @if result.items.is_empty() {
 tr {
 td colspan="13" class="text-center text-muted py-8" {
 "暂无出库记录"
 }
 }
 }
 }
 }
 }
 (pagination(StockOutListPath::PATH, query, result.total, result.page, result.total_pages))
 }
 }
}

fn stock_out_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
 operator_names: &HashMap<i64, String>,
 wh_names: &HashMap<i64, String>,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 params: &StockOutQueryParams,
) -> Markup {
 let query = build_query_string(params);
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "SalesShipment".into(), label: "销售出库", count: None },
 TabItem { value: "MaterialIssue".into(), label: "生产领料", count: None },
 ];

 let selected_type = params.transaction_type.as_deref().unwrap_or("");

 html! {
 div class="stockout-list-panel" {
 // ── Stat Cards ──
 div class="grid grid-cols-4 gap-5 mb-6" {
 (stat_card(&icon::upload_icon("w-[22px] h-[22px]"), "bg-[#fff1f0] text-danger", &total_count.to_string(), "本月出库单"))
 (stat_card(&icon::currency_icon("w-[22px] h-[22px]"), "bg-[#fff1f0] text-danger", "—", "出库总金额"))
 (stat_card(&icon::clock_icon("w-[22px] h-[22px]"), "bg-[#fff7e6] text-warn", "—", "待拣货"))
 (stat_card(&icon::check_circle_icon("w-[22px] h-[22px]"), "bg-[#f0fff0] text-success", &total_count.to_string(), "已完成"))
 }

 (status_tabs_with_param(StockOutListPath::PATH, "#stockout-data-card", "#stockout-filter-form", tabs, selected_type, "transaction_type"))

 // ── Filter Bar ──
 form class="flex items-center gap-3 mb-6 flex-wrap" id="stockout-filter-form"
 hx-get=(StockOutListPath::PATH)
 hx-trigger="change,keyup changed delay:300ms from:.search-input"
 hx-target="#stockout-data-card"
 hx-select="#stockout-data-card"
 hx-swap="outerHTML"
 hx-include="#stockout-filter-form"
 hx-push-url="true" {
 div class="relative w-60 [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="doc_number"
 placeholder="搜索单号、物料编码…"
 value=(params.doc_number.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="warehouse_id" {
 option value="" selected[params.warehouse_id.is_none()] { "全部仓库" }
 @for wh in warehouses {
 option value=(wh.id) selected[params.warehouse_id == Some(wh.id)] { (wh.name) }
 }
 }
 input class="w-[140px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" type="date" name="date_start" value=(params.date_start.as_deref().unwrap_or("")) {};
 span class="text-muted leading-9" { "~" }
 input class="w-[140px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" type="date" name="date_end" value=(params.date_end.as_deref().unwrap_or("")) {};
 }

 // ── Data Table ──
 (stock_out_data_card(result, operator_names, wh_names, &query))

 // ── Info Note ──
 div class="mt-6 p-4 px-5 flex items-start gap-3 rounded-md bg-danger-bg border border-[rgba(255,77,79,0.15)]" {
 (icon::circle_alert_icon("w-[18px] h-[18px]"))
 div class="text-sm text-fg-2 leading-relaxed" {
 strong { "出库流程说明：" }
 "出库操作通过 InventoryTransactionService.record() 执行，单据号格式为 CK-YYYY-MM-SEQ（如 CK-2026-06-000001）。"
 "销售出库消耗 SOFT预留，生产领料消耗 HARD预留。"
 "出库前系统根据拣货策略（FIFO/FEFO/最短路径/整托优先）自动分配拣货路径，出库后扣减库存并记录事务日志。"
 }
 }
 }
 }
}
