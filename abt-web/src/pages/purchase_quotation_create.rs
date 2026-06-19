use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::quotation::model::*;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_quotation::{
 PQCreatePath, PQDetailPath, PQItemRowPath, PQListPath, PQProductsPath,
 PQSupplierContactsPath,
};
use crate::utils::RequestContext;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──


// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PQCreateForm {
 pub supplier_id: i64,
 pub quotation_date: String,
 pub valid_from: String,
 pub valid_until: String,
 pub currency: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub buyer_id: Option<i64>,
 pub remark: Option<String>,
 pub items_json: String,
 pub action: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
 product_id: String,
 unit_price: String,
 min_order_qty: Option<String>,
 lead_time_days: Option<String>,
 currency: Option<String>,
 is_preferred: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_create(
 _path: PQCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;
 let supplier_svc = state.supplier_service();
 let user_svc = state.user_service();

 let suppliers = supplier_svc
 .list(
 &service_ctx,
 &mut conn,
 SupplierQuery {
 name: None,
 status: None,
 category: None,
 },
 PageParams::new(1, 200),
 )
 .await?;

 let users = user_svc
 .list_users(&service_ctx, &mut conn, 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = pq_create_page(&suppliers.items, &users);
 let page_html = admin_page(
 is_htmx,
 "新建采购报价",
 &claims,
 "purchase",
 PQCreatePath::PATH,
 "采购管理",
 Some("新建采购报价"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: search products → return HTML fragment

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_item_row(
 ctx: RequestContext,
 Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.product_service();
 let product = svc
 .get(&service_ctx, &mut conn, params.product_id)
 .await?;
 Ok(Html(item_row_fragment(&product).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 product_id: i64,
}

/// HTMX: return supplier contact info fragment (contact, phone, address)
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_supplier_contacts(
 ctx: RequestContext,
 Query(params): Query<SupplierContactParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let supplier_svc = state.supplier_service();

 let contacts = if params.supplier_id > 0 {
 supplier_svc
 .list_contacts(&service_ctx, &mut conn, params.supplier_id)
 .await
 .unwrap_or_default()
 } else {
 vec![]
 };

 // Find primary contact, or fall back to first
 let primary = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());

 let contact_name = primary.map(|c| c.name.as_str()).unwrap_or("");
 let contact_phone = primary
 .and_then(|c| c.phone.as_deref())
 .unwrap_or("");

 Ok(Html(
 supplier_contact_fields_fragment(contact_name, contact_phone).into_string(),
 ))
}

#[derive(Debug, Deserialize)]
pub struct SupplierContactParams {
 pub supplier_id: i64,
}

/// POST: create purchase quotation from form submission (HTMX)
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn create_pq(
 _path: PQCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PQCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.purchase_quotation_service();

 let quotation_date = chrono::NaiveDate::parse_from_str(&form.quotation_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效报价日期格式: {e}")))?;
 let valid_from = chrono::NaiveDate::parse_from_str(&form.valid_from, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效生效日期格式: {e}")))?;
 let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效失效日期格式: {e}")))?;

 let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

 let items: Vec<CreateQuotationItemRequest> = web_items
 .into_iter()
 .enumerate()
 .map(|(idx, item)| CreateQuotationItemRequest {
 product_id: item.product_id.parse().unwrap_or(0),
 line_no: (idx as i32) + 1,
 unit_price: item
 .unit_price
 .parse()
 .unwrap_or(rust_decimal::Decimal::ZERO),
 min_order_qty: item.min_order_qty.and_then(|s| s.parse().ok()),
 lead_time_days: item.lead_time_days.and_then(|s| s.parse().ok()),
 currency: item.currency.unwrap_or_else(|| "CNY".to_string()),
 is_preferred: item.is_preferred.is_some(),
 })
 .collect();

 let create_req = CreatePurchaseQuotationRequest {
 supplier_id: form.supplier_id,
 quotation_date,
 valid_from,
 valid_until,
 remark: form.remark.unwrap_or_default(),
 items,
 };

 let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

 let redirect = PQDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pq_create_page(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 users: &[abt_core::shared::identity::model::User],
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 let default_valid = chrono::Local::now()
 .checked_add_days(chrono::Days::new(30))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();

 html! {
 div id="pq-app" {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PQListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回采购报价列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购报价" }
 }

 form id="pq-form"
 hx-post=(PQCreatePath::PATH)
 hx-swap="none" {
 input type="hidden" id="items-json" name="items_json" value="[]";
 input type="hidden" id="form-action" name="action" value="submit";

 // ── Supplier Selection ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "供应商信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "供应商" span class="text-danger" { "*" } }
 select name="supplier_id" required
 hx-get=(PQSupplierContactsPath::PATH)
 hx-trigger="change"
 hx-target="#supplier-contact-fields"
 hx-swap="innerHTML"
 hx-vals="js:{supplier_id: this.value}" {
 option value="" disabled selected { "请选择供应商" }
 @for s in suppliers {
 option value=(s.id) { (s.name) }
 }
 }
 }
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" id="supplier-contact-fields" {
 div class="form-field" {
 label { "联系人" }
 input type="text" readonly placeholder="—" class="bg-surface" {}
 }
 div class="form-field" {
 label { "联系电话" }
 input type="text" readonly placeholder="—" class="bg-surface" {}
 }
 }
 }

 // ── Quote Info ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "报价信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "报价日期" }
 input type="date" name="quotation_date" value=(today) readonly {}
 }
 div class="form-field" {
 label { "生效日期" span class="text-danger" { "*" } }
 input type="date" name="valid_from" id="f-valid-from" value=(today) {}
 }
 div class="form-field" {
 label { "失效日期" span class="text-danger" { "*" } }
 input type="date" name="valid_until" id="f-valid-until" value=(default_valid) {}
 }
 div class="form-field" {
 label { "币种" }
 select name="currency" {
 option value="CNY" selected { "CNY (人民币)" }
 option value="USD" { "USD (美元)" }
 option value="EUR" { "EUR (欧元)" }
 }
 }
 div class="form-field" {
 label { "采购员" }
 select name="buyer_id" {
 option value="" { "请选择采购员" }
 @for u in users {
 @if u.is_active {
 option value=(u.user_id) { (u.display_name.as_deref().unwrap_or(&u.username)) }
 }
 }
 }
 }
 }
 }

 // ── Line Items ──
 div class="data-card" class="p-0 overflow-hidden mb-4" {
 div class="flex justify-between items-center" class="px-5 pt-5 pb-3" {
 span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" class="m-0 p-0 border-none" { "报价产品明细" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加产品"
 }
 }
 div class="overflow-x-auto" {
 table class="data-table" class="min-w-[900px]" {
 thead {
 tr {
 th class="w-9 text-center" { "#" }
 th { "产品编码" }
 th { "产品名称" }
 th class="w-[120px] text-right" { "单价" }
 th class="w-[100px] text-right" { "最小订购量" }
 th class="w-[90px] text-right" { "交货天数" }
 th class="w-[80px] text-center" { "币种" }
 th class="text-center" class="w-14" { "首选" }
 th class="w-9" { }
 }
 }
 tbody id="pq-item-tbody" { }
 }
 }
 div class="p-3 flex items-center gap-2" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加产品行"
 }
 }
 }

 // ── Remark ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "备注" }
 textarea name="remark" placeholder="输入报价相关备注信息…" class="w-full resize-y" class="rounded-sm" class="min-h-[80px] border border-border text-sm" style="padding:8px 12px;font-family:inherit" {}
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", PQListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click set #form-action's value to 'draft' then call document.querySelector('#pq-form').requestSubmit()" {
 "保存草稿"
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "提交报价"
 (maud::PreEscaped(r#"<script>document.currentScript.parentElement.addEventListener('click', function() {
 var items = [];
 document.querySelectorAll('#pq-item-tbody tr').forEach(function(row) {
 var vals = {};
 row.querySelectorAll('input,select').forEach(function(el) {
 if (el.name && el.name.startsWith('item_')) vals[el.name.replace('item_','')] = el.value;
 });
 items.push(vals);
 });
 if (items.length === 0) {
 show_error_toast('请至少添加一个报价产品明细');
 return;
 }
 document.querySelector('#items-json').value = JSON.stringify(items);
 document.querySelector('#pq-form').requestSubmit();
})</script>"#))
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", PQItemRowPath::PATH, "pq-item-tbody"))

 }
 }
}

/// Fragment returned by HTMX for supplier contact fields
fn supplier_contact_fields_fragment(contact_name: &str, contact_phone: &str) -> Markup {
 html! {
 div class="form-field" {
 label { "联系人" }
 input type="text" readonly value=(contact_name) placeholder="—" class="bg-surface" {}
 }
 div class="form-field" {
 label { "联系电话" }
 input type="text" readonly value=(contact_phone) placeholder="—" class="bg-surface" {}
 }
 }
}

fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
 tr {
 td class="text-muted text-xs text-center" { }
 td class="font-mono tabular-nums" { (product.product_code) }
 td { (product.pdt_name) }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" placeholder="0.00" class="w-[110px] text-right text-[13px] font-mono" class="rounded-sm" class="px-2 py-[5px] border border-border" name="item_unit_price" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0" placeholder="—" class="w-[90px] text-right text-[13px] font-mono" class="rounded-sm" class="px-2 py-[5px] border border-border" name="item_min_order_qty" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0" placeholder="—" class="w-[80px] text-right text-[13px] font-mono" class="rounded-sm" class="px-2 py-[5px] border border-border" name="item_lead_time_days" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" class="text-center text-[13px]" class="rounded-sm" class="px-2 py-[5px] border border-border" style="width:70px" name="item_currency" value="CNY" {} }
 td class="text-center" { input type="checkbox" name="item_is_preferred" class="cursor-pointer" style="width:16px;height:16px;accent-color:var(--primary)" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/>" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="item_product_id" value=(product.product_id) {}
 }
 }
}
