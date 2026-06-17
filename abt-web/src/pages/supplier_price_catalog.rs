use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;

use abt_core::purchase::supplier_price::{
 PriceListQuery, PriceUpsertRequest, PriceView, SupplierPriceService,
};
use abt_core::shared::types::{DomainError, PageParams, PaginatedResult};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::supplier_price_catalog::{
 PriceCreatePath, PriceDeletePath, PriceEditPath, SupplierPricesPath,
};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ListQuery {
 pub keyword: Option<String>,
 pub currency_code: Option<String>,
 #[serde(default)]
 pub is_active: Option<String>, // "true" / "false" / absent = all
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Form data ──

#[derive(Debug, Deserialize)]
pub struct PriceFormData {
 pub supplier_id: String,
 pub product_id: String,
 pub price: String,
 pub currency_code: String,
 pub min_order_qty: String,
 pub discount_pct: String,
 pub lead_time_days: String,
 pub tax_rate_id: Option<String>,
 pub valid_from: Option<String>,
 pub valid_until: Option<String>,
 pub sequence: String,
 pub supplier_item_code: Option<String>,
 pub supplier_item_name: Option<String>,
 #[serde(default)]
 pub is_active: Option<String>, // "on" when checked
}

fn parse_price_form(form: &PriceFormData) -> std::result::Result<PriceUpsertRequest, DomainError> {
 let parse_dec =
 |s: &str, name: &str| -> std::result::Result<rust_decimal::Decimal, DomainError> {
 s.parse()
 .map_err(|_| DomainError::validation(format!("无效{name}")))
 };
 let parse_i32 = |s: &str, name: &str| -> std::result::Result<i32, DomainError> {
 s.parse()
 .map_err(|_| DomainError::validation(format!("无效{name}")))
 };
 let parse_date = |s: &str| -> Option<chrono::NaiveDate> { s.parse().ok() };

 Ok(PriceUpsertRequest {
 supplier_id: form
 .supplier_id
 .parse()
 .map_err(|_| DomainError::validation("无效供应商ID"))?,
 product_id: form
 .product_id
 .parse()
 .map_err(|_| DomainError::validation("无效产品ID"))?,
 price: parse_dec(&form.price, "价格")?,
 currency_code: form.currency_code.clone(),
 min_order_qty: parse_dec(&form.min_order_qty, "起订量")?,
 discount_pct: parse_dec(&form.discount_pct, "折扣百分比")?,
 lead_time_days: parse_i32(&form.lead_time_days, "交货天数")?,
 tax_rate_id: form.tax_rate_id.as_deref().and_then(|s| s.parse().ok()),
 valid_from: form.valid_from.as_deref().and_then(parse_date),
 valid_until: form.valid_until.as_deref().and_then(parse_date),
 sequence: parse_i32(&form.sequence, "排序")?,
 supplier_item_code: form.supplier_item_code.clone().filter(|s| !s.is_empty()),
 supplier_item_name: form.supplier_item_name.clone().filter(|s| !s.is_empty()),
 is_active: form.is_active.as_deref() == Some("on"),
 })
}

// ══════════════════════════════════════════════════════════════════
// Handlers
// ══════════════════════════════════════════════════════════════════

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_list(
 _path: SupplierPricesPath,
 ctx: RequestContext,
 Query(params): Query<ListQuery>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 claims,
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_price_service();

 let page = params.page.unwrap_or(1);
 let filter = PriceListQuery {
 keyword: params.keyword.clone(),
 currency_code: params.currency_code.clone(),
 is_active: params.is_active.as_deref().and_then(|s| s.parse().ok()),
 supplier_id: None,
 product_id: None,
 };
 let result = svc
 .list_prices(
 &service_ctx,
 &mut conn,
 filter,
 PageParams {
 page,
 page_size: 20,
 },
 )
 .await
 .unwrap_or_else(|_| PaginatedResult::empty(page, 20));

 let content = list_page(&result, &params);
 let page_html = admin_page(
 is_htmx,
 "供应商价格管理2",
 &claims,
 "purchase",
 SupplierPricesPath::PATH,
 "采购管理",
 Some("供应商价格"),
 content,
 &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

// ── Modal: create form ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_create_modal(
 _path: PriceCreatePath,
 _ctx: RequestContext,
) -> Result<Html<String>> {
 let html = price_form(SupplierPricesPath::PATH, None);
 Ok(Html(html.into_string()))
}

// ── Modal: edit form ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_edit_modal(path: PriceEditPath, ctx: RequestContext) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_price_service();
 let price = svc.get_price(&service_ctx, &mut conn, path.id).await?;

 let action_url = PriceEditPath { id: path.id }.to_string();
 let html = price_form(&action_url, Some(&price));
 Ok(Html(html.into_string()))
}

// ── Create ──

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn create_price(
 _path: SupplierPricesPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PriceFormData>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_price_service();
 let req = parse_price_form(&form)?;
 svc.create_price(&service_ctx, &mut conn, req).await?;

 Ok((
 [
 ("HX-Trigger", r#"{"priceUpdated":"", "closePriceModal":""}"#),
 ("Content-Type", "text/html"),
 ],
 Html(String::new()),
 ))
}

// ── Update ──

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn update_price(
 path: PriceEditPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PriceFormData>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_price_service();
 let req = parse_price_form(&form)?;
 svc.update_price(&service_ctx, &mut conn, path.id, req)
 .await?;

 Ok((
 [
 ("HX-Trigger", r#"{"priceUpdated":"", "closePriceModal":""}"#),
 ("Content-Type", "text/html"),
 ],
 Html(String::new()),
 ))
}

// ── Delete ──

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn delete_price(path: PriceDeletePath, ctx: RequestContext) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_price_service();
 svc.delete_price(&service_ctx, &mut conn, path.id).await?;

 // Return updated table fragment wrapped in #price-data-card for hx-select
 let filter = PriceListQuery::default();
 let result = svc
 .list_prices(
 &service_ctx,
 &mut conn,
 filter,
 PageParams {
 page: 1,
 page_size: 20,
 },
 )
 .await
 .unwrap_or_else(|_| PaginatedResult::empty(1, 20));

 let html = table_fragment(&result, &ListQuery::default());
 Ok(([("Content-Type", "text/html")], Html(html.into_string())))
}

// ══════════════════════════════════════════════════════════════════
// Rendering
// ══════════════════════════════════════════════════════════════════

fn list_page(result: &PaginatedResult<PriceView>, query: &ListQuery) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6-left" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "供应商价格目录" }
 }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-get=(PriceCreatePath::PATH)
 hx-target="#price-modal"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .is-open to #price-modal" {
 "+ 新建价格"
 }
 }
 }
 (table_fragment(result, query))
 // Modal shells (empty, loaded via hx-get)
 (price_modal_shell())
 // Page-level event: refresh data card when price is created/updated
 (PreEscaped(r#"<script>
 document.body.addEventListener('priceUpdated', function() {
 var filterForm = document.getElementById('price-filter-form');
 if (filterForm) { filterForm.dispatchEvent(new Event('change', {bubbles: true})); }
 });
 </script>"#))
 }
 }
}

fn table_fragment(result: &PaginatedResult<PriceView>, query: &ListQuery) -> Markup {
 html! {
 div id="price-data-card" {
 (filter_bar(query))
 (data_card(result))
 }
 }
}

fn filter_bar(query: &ListQuery) -> Markup {
 let active_val = query.is_active.as_deref().unwrap_or("");
 html! {
 form id="price-filter-form" class="flex items-center gap-3 mb-5 flex-wrap"
 hx-get=(SupplierPricesPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#price-data-card"
 hx-select="#price-data-card"
 hx-swap="outerHTML"
 hx-push-url="true"
 hx-include="#price-filter-form" {

 div class="relative flex-1 max-w-xs" {
 span class="search-icon" { "🔍" }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
 placeholder="搜索供应商/产品名称或编码..."
 value=(query.keyword.as_deref().unwrap_or(""))
 autocomplete="off";
 }
 select name="currency_code" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" {
 option value="" selected[query.currency_code.is_none()] { "全部币种" }
 option value="CNY" selected[query.currency_code.as_deref() == Some("CNY")] { "CNY" }
 option value="USD" selected[query.currency_code.as_deref() == Some("USD")] { "USD" }
 option value="EUR" selected[query.currency_code.as_deref() == Some("EUR")] { "EUR" }
 }
 select name="is_active" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" {
 option value="" selected[active_val.is_empty()] { "全部状态" }
 option value="true" selected[active_val == "true"] { "启用" }
 option value="false" selected[active_val == "false"] { "停用" }
 }
 }
 }
}

fn data_card(result: &PaginatedResult<PriceView>) -> Markup {
 html! {
 div class="data-card" {
 @if result.items.is_empty() {
 (empty_state())
 } @else {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "供应商" }
 th { "产品" }
 th { "供应商料号" }
 th class="text-right text-[13px]" { "价格" }
 th { "币种" }
 th class="text-right text-[13px]" { "折扣%" }
 th class="text-right text-[13px]" { "起订量" }
 th class="text-right text-[13px]" { "交期(天)" }
 th { "有效期" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for price in &result.items {
 (row_tr(price))
 }
 }
 }
 }
 (pagination_bar(result))
 }
 }
 }
}

fn pagination_bar(result: &PaginatedResult<PriceView>) -> Markup {
 let total = result.total;
 let current = result.page;
 let total_pages = result.total_pages;
 if total_pages <= 1 {
 return html! {};
 }
 html! {
 div class="flex items-center justify-between py-4 px-5" {
 span { "共 " (total) " 条记录，第 " (current) "/" (total_pages) " 页" }
 div class="flex items-center justify-between py-4-pages" {
 @if current > 1 {
 (page_btn(current - 1, "«"))
 }
 @for p in page_range(current, total_pages) {
 @if p == 0 {
 button class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-bg text-fg text-sm cursor-pointer no-underline" disabled { "…" }
 } @else if p == current {
 button class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-bg text-fg text-sm cursor-pointer no-underline active" disabled { (p) }
 } @else {
 (page_btn(p, &p.to_string()))
 }
 }
 @if current < total_pages {
 (page_btn(current + 1, "»"))
 }
 }
 }
 }
}

fn page_btn(page: u32, label: &str) -> Markup {
 html! {
 button class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-bg text-fg text-sm cursor-pointer no-underline"
 hx-get=(SupplierPricesPath::PATH)
 hx-vals=(format!(r#"{{"page":{page}}}"#))
 hx-include="#price-filter-form"
 hx-target="#price-data-card"
 hx-select="#price-data-card"
 hx-swap="outerHTML" {
 (label)
 }
 }
}

fn page_range(current: u32, total: u32) -> Vec<u32> {
 if total <= 5 {
 (1..=total).collect()
 } else if current <= 3 {
 let mut r: Vec<u32> = (1..=4).collect();
 r.push(0);
 r.push(total);
 r
 } else if current >= total - 2 {
 let mut r = vec![1u32, 0];
 r.extend((total - 3)..=total);
 r
 } else {
 let mut r = vec![1u32, 0];
 r.extend((current - 1)..=(current + 1));
 r.push(0);
 r.push(total);
 r
 }
}

fn row_tr(price: &PriceView) -> Markup {
 let valid_text = match (&price.valid_from, &price.valid_until) {
 (Some(f), Some(u)) => format!("{} ~ {}", f, u),
 (Some(f), None) => format!("{} 起", f),
 (None, Some(u)) => format!("至 {}", u),
 (None, None) => "—".into(),
 };
 html! {
 tr {
 td {
 div style="font-weight:500" { (&price.supplier_name) }
 div class="text-muted" style="font-size:var(--text-xs)" { (&price.supplier_code) }
 }
 td {
 div { (&price.product_name) }
 div class="text-muted" style="font-size:var(--text-xs)" { (&price.product_code) }
 }
 td { (price.supplier_item_code.as_deref().unwrap_or("—")) }
 td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(price.price)) }
 td { (&price.currency_code) }
 td class="text-right text-[13px]" { (crate::utils::fmt_qty(price.discount_pct)) }
 td class="text-right text-[13px]" { (crate::utils::fmt_qty(price.min_order_qty)) }
 td class="text-right text-[13px]" { (price.lead_time_days) }
 td class="text-muted" style="font-size:var(--text-xs)" { (valid_text) }
 td {
 @if price.is_active {
 span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#f0fff0] text-[#389e0d]" { "启用" }
 } @else {
 span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff2f0] text-[#cf1322]" { "停用" }
 }
 }
 td {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs [&_svg]:w-4 [&_svg]:h-4"
 hx-get=(PriceEditPath { id: price.id }.to_string())
 hx-target="#price-modal"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .is-open to #price-modal" {
 "编辑"
 }
 button class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90 [&_svg]:w-4 [&_svg]:h-4"
 hx-post=(PriceDeletePath { id: price.id }.to_string())
 hx-confirm="确认删除此价格记录？"
 hx-target="#price-data-card"
 hx-select="#price-data-card"
 hx-swap="outerHTML" {
 "删除"
 }
 }
 }
 }
 }
}

fn empty_state() -> Markup {
 html! {
 div style="text-align:center;padding:var(--space-12);color:var(--text-muted)" {
 p style="margin:0;font-size:var(--text-lg)" { "暂无价格记录" }
 p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "点击「+ 新建价格」添加供应商价格" }
 }
 }
}

// ── Modal ──

fn price_modal_shell() -> Markup {
 html! {
 div class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" id="price-modal"
 _="on closePriceModal from body remove .is-open
 on click[me is event.target] remove .is-open" {
 }
 }
}

fn price_form(action_url: &str, price: Option<&PriceView>) -> Markup {
 let is_edit = price.is_some();
 let title = if is_edit {
 "编辑价格"
 } else {
 "新建价格"
 };

 // Extract values for pre-filling
 let sid = price.map(|p| p.supplier_id.to_string()).unwrap_or_default();
 let pid = price.map(|p| p.product_id.to_string()).unwrap_or_default();
 let pv = price.map(|p| p.price.to_string()).unwrap_or_default();
 let cc = price.map(|p| p.currency_code.as_str()).unwrap_or("CNY");
 let moq = price
 .map(|p| p.min_order_qty.to_string())
 .unwrap_or_else(|| "1".into());
 let disc = price
 .map(|p| p.discount_pct.to_string())
 .unwrap_or_else(|| "0".into());
 let ldt = price
 .map(|p| p.lead_time_days.to_string())
 .unwrap_or_else(|| "0".into());
 let seq = price
 .map(|p| p.sequence.to_string())
 .unwrap_or_else(|| "0".into());
 let sic = price
 .and_then(|p| p.supplier_item_code.as_deref())
 .unwrap_or("");
 let sin = price
 .and_then(|p| p.supplier_item_name.as_deref())
 .unwrap_or("");
 let vf = price
 .and_then(|p| p.valid_from.map(|d| d.to_string()))
 .unwrap_or_default();
 let vu = price
 .and_then(|p| p.valid_until.map(|d| d.to_string()))
 .unwrap_or_default();
 let tax_id = price
 .and_then(|p| p.tax_rate_id.map(|t| t.to_string()))
 .unwrap_or_default();
 let active_checked = price.map(|p| p.is_active).unwrap_or(true);

 // Display supplier/product name for edit
 let supplier_display = price
 .map(|p| format!("{} ({})", p.supplier_name, p.supplier_code))
 .unwrap_or_default();
 let product_display = price
 .map(|p| format!("{} ({})", p.product_name, p.product_code))
 .unwrap_or_default();

 html! {
 div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" _="on click halt" {
 div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
 h2 { (title) }
 button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
 _="on click remove .is-open from #price-modal" { "×" }
 }
 form hx-post=(action_url) hx-target="this" hx-swap="outerHTML"
 _="on 'htmx:afterRequest'[detail.successful] remove .is-open from #price-modal" {

 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 // Section: Basic info
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "基本信息" }
 @if is_edit {
 div class="supplier-info-bar" {
 span { "供应商: " (supplier_display) }
 span { " | 产品: " (product_display) }
 }
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商ID" span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="supplier_id"
 required value=(sid);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品ID" span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="product_id"
 required value=(pid);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "单价" span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.000001"
 name="price" required value=(pv);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "币种" }
 select name="currency_code" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" {
 @for c in &["CNY", "USD", "EUR"] {
 option value=(*c) selected[cc == *c] {
 (match *c { "CNY" => "CNY 人民币", "USD" => "USD 美元", _ => "EUR 欧元" })
 }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "起订量" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.000001"
 name="min_order_qty" value=(moq);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "折扣(%)" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.01" min="0" max="100"
 name="discount_pct" value=(disc);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "交货天数" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="1" min="0"
 name="lead_time_days" value=(ldt);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "排序" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="1"
 name="sequence" value=(seq);
 }
 }
 }

 // Section: Supplier item info
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "供应商物料信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商料号" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text"
 name="supplier_item_code" value=(sic);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商品名" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text"
 name="supplier_item_name" value=(sin);
 }
 }
 }

 // Section: Tax & validity
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "税率与有效期" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "税率ID" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="1"
 name="tax_rate_id" value=(tax_id);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "启用状态" }
 label style="display:flex;align-items:center;gap:var(--space-2);cursor:pointer" {
 input type="checkbox" name="is_active"
 checked[active_checked] {};
 " 启用"
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "生效日期" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="valid_from" value=(vf);
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "失效日期" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="valid_until" value=(vu);
 }
 }
 }
 }

 div class="px-6 py-4 [border-top:1px_solid_var(--border-soft)] flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .is-open from #price-modal" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "保存" }
 }
 }
 }
 }
}
