use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::price::ProductPriceService;
use abt_core::master_data::price::model::{PriceQuery, PriceType};
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::product::{PriceHistoryDetailPath, PriceHistoryListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PriceHistoryQueryParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_from: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_to: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Enriched Row (owned) ──

#[allow(dead_code)]
pub struct PriceHistoryRow {
 pub log_id: i64,
 pub product_id: i64,
 pub product_code: String,
 pub product_name: String,
 pub price_type: PriceType,
 pub old_price: Option<Decimal>,
 pub new_price: Decimal,
 pub operator_name: String,
 pub remark: String,
 pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_price_history_list(
 _path: PriceHistoryListPath,
 ctx: RequestContext,
 Query(params): Query<PriceHistoryQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let (rows, total, page, total_pages) = fetch_enriched_rows(&state, &service_ctx, &mut conn, &params).await?;


 let content = price_history_page(&rows, total, page, total_pages, &params);
 let page_html = admin_page(
 is_htmx,
 "价格变更记录",
 &claims,
 "md",
 PriceHistoryListPath::PATH,
 "主数据管理",
 Some("价格变更记录"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Data Fetching ──

async fn fetch_enriched_rows(
 state: &crate::state::AppState,
 service_ctx: &abt_core::shared::types::ServiceContext,
 conn: abt_core::shared::types::PgExecutor<'_>,
 params: &PriceHistoryQueryParams,
) -> crate::errors::Result<(Vec<PriceHistoryRow>, u64, u32, u32)> {
 let price_svc = state.product_price_service();
 let product_svc = state.product_service();
 let user_svc = state.user_service();
 let date_from = params.date_from.as_deref()
 .and_then(|s| s.parse::<chrono::NaiveDate>().ok())
 .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
 let date_to = params.date_to.as_deref()
 .and_then(|s| s.parse::<chrono::NaiveDate>().ok())
 .and_then(|d| d.succ_opt().and_then(|d| d.and_hms_opt(0, 0, 0)))
 .map(|d| d.and_utc());
 let query = PriceQuery {
 product_id: None,
 price_type: None,
 keyword: params.keyword.clone(),
 date_from,
 date_to,
 };
 let page_num = params.page.unwrap_or(1);
 let result = price_svc.list_price_history(service_ctx, conn, query, PageParams::new(page_num, 20)).await?;

 let total = result.total;
 let page = result.page;
 let total_pages = result.total_pages;

 // Collect unique product IDs and operator IDs
 let product_ids: Vec<i64> = result.items.iter().map(|e| e.product_id).collect();
 let operator_ids: Vec<i64> = result.items.iter()
 .filter_map(|e| e.operator_id)
 .collect();

 // Batch fetch products
 let product_map: HashMap<i64, (String, String)> = if product_ids.is_empty() {
 HashMap::new()
 } else {
 product_svc.get_by_ids(service_ctx, conn, product_ids)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|p| (p.product_id, (p.product_code, p.pdt_name)))
 .collect()
 };

 // Batch fetch users
 let user_map: HashMap<i64, String> = if operator_ids.is_empty() {
 HashMap::new()
 } else {
 user_svc.get_users_by_ids(service_ctx, conn, operator_ids)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
 .collect()
 };

 // Build enriched rows
 let rows: Vec<PriceHistoryRow> = result.items.into_iter().map(|entry| {
 let (product_code, product_name) = product_map
 .get(&entry.product_id)
 .cloned()
 .unwrap_or(("—".into(), "—".into()));
 let operator_name = entry.operator_id
 .and_then(|id| user_map.get(&id).cloned())
 .unwrap_or("—".into());
 PriceHistoryRow {
 log_id: entry.log_id,
 product_id: entry.product_id,
 product_code,
 product_name,
 price_type: entry.price_type,
 old_price: entry.old_price,
 new_price: entry.new_price,
 operator_name,
 remark: entry.remark,
 created_at: entry.created_at,
 }
 }).collect();

 Ok((rows, total, page, total_pages))
}

// ── Page ──

fn price_history_page(rows: &[PriceHistoryRow], total: u64, page: u32, total_pages: u32, params: &PriceHistoryQueryParams) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "价格变更记录" }
 }

 // ── Stats Row ──
 div class="grid gap-5" {
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" { (icon::currency_icon("w-5 h-5")) }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { (total) }
 div class="text-sm text-muted mt-1" { "总变更次数" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 green" { (icon::trending_up_icon("w-5 h-5")) }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { "—" }
 div class="text-sm text-muted mt-1" { "平均涨幅" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 orange" { (icon::clock_icon("w-5 h-5")) }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { "—" }
 div class="text-sm text-muted mt-1" { "本月变更" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 red" { (icon::lock_icon("w-5 h-5")) }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { "—" }
 div class="text-sm text-muted mt-1" { "涉及产品数" }
 }
 }
 }
 // ── Filter Bar + Table ──
 div class="customer-list-panel" {
 // ── Filter Bar ──
 form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
 hx-get=(PriceHistoryListPath::PATH)
 hx-trigger="change,keyup changed delay:300ms from:.search-input"
 hx-target=".data-card"
 hx-select=".data-card"
 hx-swap="outerHTML"
 hx-include="#filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="keyword"
 placeholder="搜索产品名称 / 编码…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_from"
 style="width:150px;padding-left:12px"
 value=(params.date_from.as_deref().unwrap_or(""))
 title="开始日期";
 span class="text-muted text-[13px]" style="line-height:36px" { "至" }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_to"
 style="width:150px;padding-left:12px"
 value=(params.date_to.as_deref().unwrap_or(""))
 title="结束日期";
 a href=(PriceHistoryListPath::PATH) class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" style="height:36px;text-decoration:none" { "重置" }
 }
 // ── Data Table ──
 (data_card(rows, total, page, total_pages))
 }

 // ── Detail Drawer Overlay ──
 div class="fixed z-[1000] opacity-0" id="detail-drawer"
 _="on click[me is event.target] remove .open" {
 div class="fixed z-[1001] w-[520px] bg-white flex flex-col" _="on click halt the event" {
 div class="flex items-center justify-between border-b border-border-soft shrink-0" {
 h2 { "变更详情" }
 button class="w-[32px] h-[32px] border-none cursor-pointer flex items-center justify-center text-muted" _="on click remove .open from #detail-drawer" {
 (icon::x_icon("w-4.5 h-4.5"))
 }
 }
 div class="flex-1 overflow-y-auto" id="detail-body" {
 }
 }
 }
 }
 }
}

fn data_card(rows: &[PriceHistoryRow], total: u64, page: u32, total_pages: u32) -> Markup {
 html! {
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" class="w-full" style="table-layout:fixed" {
 thead {
 tr {
 th class="w-10" { "#" }
 th class="w-[120px]" { "产品编码" }
 th style="width:22%" { "产品名称" }
 th class="w-[90px]" class="text-right text-[13px]" { "原价格" }
 th class="w-[90px]" class="text-right text-[13px]" { "新价格" }
 th style="width:70px" class="text-right text-[13px]" { "变动" }
 th class="w-[60px]" { "操作人" }
 th class="w-[110px]" { "变更时间" }
 th { "备注" }
 th style="width:70px" { "操作" }
 }
 }
 tbody {
 @for (i, row) in rows.iter().enumerate() {
 (price_history_row(i, row))
 }
 @if rows.is_empty() {
 tr {
 td colspan="10" class="text-center text-muted py-8" {
 "暂无价格变更记录"
 }
 }
 }
 }
 }
 }
 (pagination(PriceHistoryListPath::PATH, "", total, page, total_pages))
 }
 }
}
fn price_history_row(index: usize, row: &PriceHistoryRow) -> Markup {
 let old_str = row.old_price.map(|p| format!("{:.4}", p)).unwrap_or_else(|| "—".into());
 let new_str = format!("{:.4}", row.new_price);
 let (pct, is_up) = match row.old_price {
 Some(old) if !old.is_zero() => {
 let change = (row.new_price - old) / old * Decimal::from(100);
 let up = row.new_price >= old;
 (if change >= Decimal::ZERO { format!("+{:.1}%", change) } else { format!("{:.1}%", change) }, up)
 }
 _ => ("—".into(), true),
 };
 let tag_class = if is_up { "inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-semibold bg-success-bg text-success" } else { "inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-semibold bg-danger-bg text-danger" };
 let detail_path = PriceHistoryDetailPath { log_id: row.log_id };
 html! {
 tr class="cursor-pointer"
 hx-get=(detail_path.to_string())
 hx-target="#detail-body"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .open to #detail-drawer" {
 td class="text-muted" { (index + 1) }
 td class="font-mono tabular-nums" { (row.product_code) }
 td class="overflow-hidden whitespace-nowrap text-ellipsis" title=(row.product_name) {
 a href="#" class="text-accent cursor-pointer font-medium no-underline" onclick="event.preventDefault()" { (row.product_name) }
 }
 td class="text-right text-[13px] text-muted" { "¥ " (old_str) }
 td class="text-right text-[13px]" { strong { "¥ " (new_str) } }
 td class="text-right text-[13px]" {
 span class=(tag_class) { (pct) }
 }
 td { (row.operator_name) }
 td class="text-muted text-[13px]" { (row.created_at.format("%Y-%m-%d %H:%M")) }
 td class="overflow-hidden whitespace-nowrap text-ellipsis" title=(row.remark) {
 @if row.remark.is_empty() {
 span class="text-muted" { "—" }
 } @else {
 (row.remark)
 }
 }
 td {
 button class="inline-flex items-center gap-2 py-1 px-2.5 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-xs font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click halt the event on 'htmx:afterRequest' add .open to #detail-drawer"
 hx-get=(detail_path.to_string())
 hx-target="#detail-body"
 hx-swap="innerHTML" { "详情" }
 }
 }
 }
}
// ── Detail Drawer (HTMX) ──
#[require_permission("PRODUCT", "read")]
pub async fn get_price_history_detail(
 path: PriceHistoryDetailPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let price_svc = state.product_price_service();
 let product_svc = state.product_service();
 let user_svc = state.user_service();
 // Fetch all history entries and find the one matching log_id
 let query = PriceQuery { product_id: None, price_type: None, keyword: None, date_from: None, date_to: None };
 let result = price_svc.list_price_history(&service_ctx, &mut conn, query, PageParams::new(1, 1000)).await?;
 let entry = result.items.into_iter().find(|e| e.log_id == path.log_id)
 .ok_or_else(|| abt_core::shared::types::DomainError::NotFound("记录不存在".into()))?;
 // Enrich
 let products = product_svc.get_by_ids(&service_ctx, &mut conn, vec![entry.product_id]).await.unwrap_or_default();
 let (product_code, product_name) = products.first()
 .map(|p| (p.product_code.clone(), p.pdt_name.clone()))
 .unwrap_or(("—".into(), "—".into()));
 let operator_name = match entry.operator_id {
 Some(id) => {
 match user_svc.get_users_by_ids(&service_ctx, &mut conn, vec![id]).await {
 Ok(users) => users.into_iter().next()
 .map(|u| u.user.display_name.unwrap_or(u.user.username))
 .unwrap_or("—".into()),
 Err(_) => "—".into(),
 }
 }
 None => "—".into(),
 };
 let row = PriceHistoryRow {
 log_id: entry.log_id,
 product_id: entry.product_id,
 product_code,
 product_name,
 price_type: entry.price_type,
 old_price: entry.old_price,
 new_price: entry.new_price,
 operator_name,
 remark: entry.remark,
 created_at: entry.created_at,
 };
 Ok(Html(detail_content(&row).into_string()))
}
fn detail_content(row: &PriceHistoryRow) -> Markup {
 let old_str = row.old_price.map(|p| format!("¥ {:.4}", p)).unwrap_or_else(|| "—".into());
 let new_str = format!("¥ {:.4}", row.new_price);
 let (pct, is_up) = match row.old_price {
 Some(old) if !old.is_zero() => {
 let change = (row.new_price - old) / old * Decimal::from(100);
 let up = row.new_price >= old;
 (if change >= Decimal::ZERO { format!("+{:.1}%", change) } else { format!("{:.1}%", change) }, up)
 }
 _ => ("—".into(), true),
 };
 let tag_class = if is_up { "inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-semibold bg-success-bg text-success" } else { "inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-semibold bg-danger-bg text-danger" };
 html! {
 // ── 产品信息 ──
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::box_icon("w-4 h-4"))
 "产品信息"
 }
 div class="grid gap-5 gap-4" {
 div class="detail-flex flex-col gap-1" {
 label { "产品名称" }
 span { (row.product_name) }
 }
 div class="detail-flex flex-col gap-1" {
 label { "产品编码" }
 span class="font-mono" { (row.product_code) }
 }
 div class="detail-flex flex-col gap-1" {
 label { "价格类型" }
 span { (price_type_label(row.price_type)) }
 }
 div class="detail-flex flex-col gap-1" {
 label { "操作人" }
 span { (row.operator_name) }
 }
 }
 }
 // ── 价格变动 ──
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::currency_icon("w-4 h-4"))
 "价格变动"
 }
 div class="bg-accent-50 flex items-center gap-4 rounded-md" {
 div class="text-center" {
 div class="text-xs text-muted mb-1" { "原价格" }
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (old_str) }
 }
 div class="text-accent text-[20px]" {
 (icon::arrow_right_icon("w-6 h-6"))
 }
 div class="text-center" {
 div class="text-xs text-muted mb-1" { "新价格" }
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (new_str) }
 }
 div class="ml-auto" {
 span class=(format!("{} px-3 py-1 text-sm", tag_class)) { (pct) }
 }
 }
 }
 // ── 调价说明 ──
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::comment_icon("w-4 h-4"))
 "调价说明"
 }
 div class="bg-surface text-sm text-fg border border-border p-3 rounded-md" {
 @if row.remark.is_empty() { "—" } @else { (row.remark) }
 }
 }
 // ── 变更时间 ──
 div class="mb-5" {
 div class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" {
 (icon::clock_icon("w-4 h-4"))
 "变更时间"
 }
 div class="text-fg font-medium text-[15px]" {
 (row.created_at.format("%Y-%m-%d %H:%M"))
 }
 }
 }
}
fn price_type_label(pt: PriceType) -> &'static str {
 match pt {
 PriceType::Purchase => "采购价",
 PriceType::Sales => "销售价",
 PriceType::StandardCost => "标准成本",
 }
}