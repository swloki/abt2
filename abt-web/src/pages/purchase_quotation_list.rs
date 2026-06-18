use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseQuotationStatus;
use abt_core::purchase::quotation::model::*;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_quotation::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PQQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub supplier_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_range: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
 let today = chrono::Local::now().date_naive();
 match range {
 "7d" => (Some(today - chrono::Days::new(7)), None),
 "30d" => (Some(today - chrono::Days::new(30)), None),
 "3m" => (Some(today - chrono::Months::new(3)), None),
 _ => (None, None),
 }
}

fn build_filter(params: &PQQueryParams) -> PurchaseQuotationQuery {
 let (quotation_date_start, quotation_date_end) = params
 .date_range
 .as_deref()
 .map(parse_date_range)
 .unwrap_or((None, None));
 PurchaseQuotationQuery {
 supplier_id: params.supplier_id,
 status: params.status.and_then(PurchaseQuotationStatus::from_i16),
 quotation_date_start,
 quotation_date_end,
 }
}

fn build_query_string(params: &PQQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref kw) = params.keyword {
 q.push(format!("keyword={kw}"));
 }
 if let Some(s) = params.status {
 q.push(format!("status={s}"));
 }
 if let Some(sid) = params.supplier_id {
 q.push(format!("supplier_id={sid}"));
 }
 if let Some(ref dr) = params.date_range {
 q.push(format!("date_range={dr}"));
 }
 q.join("&")
}

async fn resolve_supplier_names<S: SupplierService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 quotations: &[PurchaseQuotation],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = quotations.iter().map(|q| q.supplier_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 let all = svc
 .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 200))
 .await;
 match all {
 Ok(result) => result
 .items
 .into_iter()
 .filter(|s| ids.contains(&s.id))
 .map(|s| (s.id, s.name))
 .collect(),
 Err(_) => HashMap::new(),
 }
}

async fn resolve_supplier_contacts<S: SupplierService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 quotations: &[PurchaseQuotation],
) -> HashMap<i64, String> {
 let unique_ids: Vec<i64> = quotations
 .iter()
 .map(|q| q.supplier_id)
 .collect::<std::collections::HashSet<_>>()
 .into_iter()
 .collect();
 if unique_ids.is_empty() {
 return HashMap::new();
 }
 let mut map = HashMap::new();
 for sid in &unique_ids {
 if let Ok(contacts) = svc.list_contacts(ctx, db, *sid).await {
 if let Some(primary) = contacts.iter().find(|c| c.is_primary) {
 map.insert(*sid, primary.name.clone());
 } else if let Some(first) = contacts.first() {
 map.insert(*sid, first.name.clone());
 }
 }
 }
 map
}

async fn resolve_supplier_currencies<S: SupplierService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 quotations: &[PurchaseQuotation],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = quotations.iter().map(|q| q.supplier_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 let all = svc
 .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 200))
 .await;
 match all {
 Ok(result) => result
 .items
 .into_iter()
 .filter(|s| ids.contains(&s.id))
 .map(|s| (s.id, s.currency))
 .collect(),
 Err(_) => HashMap::new(),
 }
}

// ── Status Labels ──

fn status_label(s: PurchaseQuotationStatus) -> (&'static str, &'static str) {
 match s {
 PurchaseQuotationStatus::Draft => ("草稿", "status-draft"),
 PurchaseQuotationStatus::Active => ("已生效", "status-confirmed"),
 PurchaseQuotationStatus::Expired => ("已过期", "status-cancelled"),
 PurchaseQuotationStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

// ── Handlers ──

#[require_permission("PURCHASE_QUOTATION", "read")]
pub async fn get_pq_list(
 _path: PQListPath,
 ctx: RequestContext,
 Query(params): Query<PQQueryParams>,
) -> Result<Html<String>> {
 let can_create = ctx.has_permission("PURCHASE_QUOTATION", "create").await;
 let can_delete = ctx.has_permission("PURCHASE_QUOTATION", "delete").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_quotation_service();
 let supplier_svc = state.supplier_service();

 let filter = build_filter(&params);
 let page = PageParams::new(params.page.unwrap_or(1), 20);
 let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

 let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
 let supplier_contacts = resolve_supplier_contacts(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
 let supplier_currencies = resolve_supplier_currencies(&supplier_svc, &service_ctx, &mut conn, &result.items).await;

 let suppliers = supplier_svc
 .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
 .await?;

 let content = pq_list_page(&result, &supplier_names, &supplier_contacts, &supplier_currencies, &suppliers.items, &params, can_create, can_delete);
 let page_html = admin_page(
 is_htmx, "采购报价", &claims, "purchase", PQListPath::PATH, "采购管理", Some("采购报价"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn pq_list_page(
 result: &abt_core::shared::types::PaginatedResult<PurchaseQuotation>,
 supplier_names: &HashMap<i64, String>,
 supplier_contacts: &HashMap<i64, String>,
 supplier_currencies: &HashMap<i64, String>,
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 params: &PQQueryParams,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "采购报价" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(PQCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建采购报价"
 }
 }
 }
 }

 // ── Tabs + Filter + Data Table (HTMX panel) ──
 (pq_table_fragment(result, supplier_names, supplier_contacts, supplier_currencies, suppliers, params, can_delete))
 }
 }
}

fn pq_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<PurchaseQuotation>,
 supplier_names: &HashMap<i64, String>,
 supplier_contacts: &HashMap<i64, String>,
 supplier_currencies: &HashMap<i64, String>,
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 params: &PQQueryParams,
 can_delete: bool,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: None },
 TabItem { value: "2".into(), label: "已生效", count: None },
 TabItem { value: "3".into(), label: "已过期", count: None },
 TabItem { value: "4".into(), label: "已取消", count: None },
 ];

 let selected_supplier = params.supplier_id.map(|id| id.to_string()).unwrap_or_default();
 let selected_range = params.date_range.as_deref().unwrap_or("");

 html! {
 div class="pq-list-panel" {
 (status_tabs_with_param(PQListPath::PATH, "#pq-data-card", "#pq-filter-form", tabs, &active_value, "status"))

 // ── Filter Bar ──
 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="pq-filter-form"
 hx-get=(PQListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#pq-data-card"
 hx-select="#pq-data-card"
 hx-swap="outerHTML"
 hx-select-oob="#status-tabs"
 hx-include="#pq-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
 placeholder="搜索报价单号…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="supplier_id" {
 option value="" { "全部供应商" }
 @for s in suppliers {
 option value=(s.id) selected[selected_supplier == s.id.to_string()] { (s.name) }
 }
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="date_range" {
 option value="" selected[selected_range.is_empty()] { "报价日期" }
 option value="7d" selected[selected_range == "7d"] { "最近7天" }
 option value="30d" selected[selected_range == "30d"] { "最近30天" }
 option value="3m" selected[selected_range == "3m"] { "最近3个月" }
 }
 }

 // ── Data Table ──
 div class="data-card" id="pq-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "报价单号" }
 th { "供应商名称" }
 th { "联系人" }
 th { "状态" }
 th { "报价日期" }
 th { "有效期至" }
 th { "币种" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for q in &result.items {
 (pq_row(q, supplier_names, supplier_contacts, supplier_currencies, can_delete))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="8" class="text-center text-muted py-8" {
 "暂无报价数据"
 }
 }
 }
 }
 }
 }
 (pagination(PQListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
 }
}

fn pq_row(
 q: &PurchaseQuotation,
 supplier_names: &HashMap<i64, String>,
 supplier_contacts: &HashMap<i64, String>,
 supplier_currencies: &HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 let detail_path = PQDetailPath { id: q.id };
 let (status_text, status_class) = status_label(q.status);
 let supplier_name = supplier_names.get(&q.supplier_id).map(|s| s.as_str()).unwrap_or("—");
 let contact = supplier_contacts.get(&q.supplier_id).map(|s| s.as_str()).unwrap_or("—");
 let currency = supplier_currencies.get(&q.supplier_id).map(|s| s.as_str()).unwrap_or("CNY");
 let onclick = format!("location.href='{}'", detail_path);
 let is_draft = q.status == PurchaseQuotationStatus::Draft;
 let status_allows_delete = q.status != PurchaseQuotationStatus::Active;
 html! {
 tr class="cursor-pointer" {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) { (q.doc_number) }
 td onclick=(&onclick) { (supplier_name) }
 td onclick=(&onclick) { (contact) }
 td onclick=(&onclick) {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 td class="font-mono tabular-nums" onclick=(&onclick) { (q.quotation_date.format("%Y-%m-%d")) }
 td class="font-mono tabular-nums" onclick=(&onclick) { (q.valid_until.format("%Y-%m-%d")) }
 td onclick=(&onclick) { (currency) }
 td _="on click halt the event" {
 @if is_draft || (can_delete && status_allows_delete) {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 @if is_draft {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(detail_path.to_string()) title="编辑" {
 (icon::edit_icon("w-4 h-4"))
 }
 }
 @if can_delete && status_allows_delete {
 button class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer row-action-danger"
 title="删除"
 hx-post=(PQDeletePath { id: q.id }.to_string())
 hx-confirm="确认删除此报价？" {
 (icon::trash_icon("w-4 h-4"))
 }
 }
 }
 }
 }
 }
 }
}
