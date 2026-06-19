use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::enums::PurchaseQuotationStatus;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::*;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::TaxRateService;
use abt_core::purchase::quotation::model::PurchaseQuotationQuery;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::RequestContext;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
 pub name: Option<String>,
 pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SupplierDetailParams {
 pub supplier_id: i64,
}

// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct POCreateForm {
 pub supplier_id: i64,
 pub order_date: String,
 pub expected_delivery_date: Option<String>,
 pub payment_terms: Option<String>,
 pub currency: Option<String>,
 pub delivery_address: Option<String>,
 pub related_quotation_id: Option<String>,
 pub buyer_id: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
 product_id: String,
 description: Option<String>,
 quantity: String,
 unit_price: String,
 item_delivery_date: Option<String>,
 discount_pct: Option<String>,
 tax_rate_id: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_create(
 _path: POCreatePath,
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
 let pq_svc = state.purchase_quotation_service();

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
 .await?;

 let quotations = pq_svc
 .list(
 &service_ctx,
 &mut conn,
 PurchaseQuotationQuery {
 supplier_id: None,
 status: Some(PurchaseQuotationStatus::Active),
 quotation_date_start: None,
 quotation_date_end: None,
 },
 PageParams::new(1, 200),
 )
 .await?;

 let tax_rates = state.tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();

 let content = po_create_page(&suppliers.items, &users.items, &quotations.items, &tax_rates);
 let page_html = admin_page(
 is_htmx,
 "新建采购订单",
 &claims,
 "purchase",
 POCreatePath::PATH,
 "采购管理",
 Some("新建采购订单"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: return supplier detail fragment (contact/phone/address/info bar)
#[require_permission("SUPPLIER", "read")]
pub async fn get_po_supplier_detail(
 ctx: RequestContext,
 Query(params): Query<SupplierDetailParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_service();

 let supplier = svc.get(&service_ctx, &mut conn, params.supplier_id).await?;
 let contacts = svc
 .list_contacts(&service_ctx, &mut conn, params.supplier_id)
 .await
 .unwrap_or_default();

 let primary = contacts.iter().find(|c| c.is_primary);
 let contact_name = primary
 .map(|c| c.name.as_str())
 .unwrap_or("—");
 let contact_phone = primary
 .and_then(|c| c.phone.as_deref())
 .unwrap_or("—");

 // Compute cooperation years from created_at
 let coop_years = {
 let created = supplier.created_at;
 let now = chrono::Utc::now();
 let diff = now.signed_duration_since(created);
 diff.num_days() / 365
 };

 Ok(Html(
 supplier_detail_fragment(contact_name, contact_phone, coop_years).into_string(),
 ))
}


/// HTMX/JS: return active tax rates as JSON
#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_tax_rates(ctx: RequestContext) -> Result<axum::Json<serde_json::Value>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let rates = state.tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();
 let json: Vec<serde_json::Value> = rates.iter().map(|r| serde_json::json!({
 "id": r.id, "code": r.code, "name": r.name, "rate": r.rate.to_string()
 })).collect();
 Ok(axum::Json(serde_json::Value::Array(json)))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 product_id: i64,
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_item_row(
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
 let tax_rates = state.tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();
 Ok(Html(item_row_fragment(&product, &tax_rates).into_string()))
}

/// POST: create purchase order from form submission (HTMX)
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn create_po(
 _path: POCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<POCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.purchase_order_service();

 let order_date = chrono::NaiveDate::parse_from_str(&form.order_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效订单日期格式: {e}")))?;

 let expected_delivery_date = form
 .expected_delivery_date
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|s| {
 chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效预期交货日期格式: {e}")))
 })
 .transpose()?;

 let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

 let items: Vec<CreateOrderItemRequest> = web_items
 .into_iter()
 .enumerate()
 .map(|(idx, item)| {
 let item_expected_delivery_date = item
 .item_delivery_date
 .as_deref()
 .filter(|s| !s.is_empty())
 .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

 let quantity: rust_decimal::Decimal = item
 .quantity
 .parse()
 .map_err(|_| DomainError::validation(format!("第 {} 行无效数量", idx + 1)))?;
 let unit_price: rust_decimal::Decimal = item
 .unit_price
 .parse()
 .map_err(|_| DomainError::validation(format!("第 {} 行无效单价", idx + 1)))?;

 Ok(CreateOrderItemRequest {
 product_id: item.product_id.parse().unwrap_or(0),
 line_no: (idx as i32) + 1,
 description: item.description.unwrap_or_default(),
 quantity,
 unit_price,
 quotation_item_id: None,
 expected_delivery_date: item_expected_delivery_date,
 discount_pct: item.discount_pct.as_deref()
 .and_then(|s| s.parse().ok())
 .unwrap_or(rust_decimal::Decimal::ZERO),
 tax_rate_id: item.tax_rate_id.as_deref()
 .and_then(|s| s.parse().ok())
 .filter(|&v: &i64| v > 0),
 })
 })
 .collect::<Result<Vec<_>, DomainError>>()?;

 let create_req = CreatePurchaseOrderRequest {
 supplier_id: form.supplier_id,
 order_date,
 expected_delivery_date,
 payment_terms: form.payment_terms,
 delivery_address: form.delivery_address,
 remark: form.remark.unwrap_or_default(),
 currency_code: form.currency.unwrap_or_else(|| String::from("CNY")),
 currency_rate: rust_decimal::Decimal::ONE,
 discount_amount: rust_decimal::Decimal::ZERO,
 items,
 };

 let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

 let redirect = PODetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn po_create_page(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 users: &[abt_core::shared::identity::model::User],
 quotations: &[abt_core::purchase::quotation::model::PurchaseQuotation],
 tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 let default_delivery = chrono::Local::now()
 .checked_add_days(chrono::Days::new(15))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();

 html! {
 div id="po-app" {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", POListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回采购订单列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购订单" }
 }

 form id="po-form"
 hx-post=(POCreatePath::PATH)
 hx-swap="none" {
 input type="hidden" id="items-json" name="items_json" value="[]";

 // ── Supplier Selection ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "供应商信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "供应商" span class="text-danger" { "*" } }
 select name="supplier_id" required
 hx-get=(POSupplierDetailPath::PATH)
 hx-trigger="change"
 hx-target="#supplier-detail"
 hx-swap="innerHTML"
 hx-include="this" {
 option value="" disabled selected { "请选择供应商" }
 @for s in suppliers {
 option value=(s.id) { (s.name) }
 }
 }
 }
 div class="form-field" {
 label { "联系人" }
 input type="text" id="supplier-contact" readonly placeholder="自动填充" class="bg-surface" {}
 }
 div class="form-field" {
 label { "联系电话" }
 input type="text" id="supplier-phone" readonly placeholder="自动填充" class="bg-surface" {}
 }
 div class="form-field col-span-2" {
 label { "供应商地址" }
 input type="text" id="supplier-address" readonly placeholder="自动填充" class="bg-surface" {}
 }
 }
 // ── Supplier Info Bar ──
 div id="supplier-detail" class="mt-3" { }
 }

 // ── Order Info ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "订单信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "订单日期" }
 input type="date" name="order_date" value=(today) readonly {}
 }
 div class="form-field" {
 label { "预期交货日期" }
 input type="date" name="expected_delivery_date" value=(default_delivery) {}
 }
 div class="form-field" {
 label { "付款条件" }
 select name="payment_terms" {
 option value="" { "请选择付款条件" }
 option value="30天净额" { "30天净额" }
 option value="60天净额" { "60天净额" }
 option value="预付30%" { "预付30%" }
 option value="货到付款" { "货到付款" }
 option value="月结30天" { "月结30天" }
 }
 }
 div class="form-field" {
 label { "币种" }
 select name="currency" {
 option value="CNY" selected { "CNY" }
 option value="USD" { "USD" }
 option value="EUR" { "EUR" }
 }
 }
 div class="form-field col-span-2" {
 label { "交货地址" }
 input type="text" name="delivery_address" placeholder="输入交货地址…" {}
 }
 div class="form-field" {
 label { "关联报价" }
 select name="related_quotation_id" {
 option value="" { "请选择采购报价" }
 @for q in quotations {
 option value=(q.id) { (q.doc_number) }
 }
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
 div class="form-field col-span-2" {
 label { "备注" }
 textarea name="remark" placeholder="输入订单相关备注信息…" class="w-full resize-y" class="rounded-sm" class="min-h-[80px] border border-border text-sm" style="padding:8px 12px;font-family:inherit" {}
 }
 }
 }

 // ── Line Items ──
 div class="data-card" class="p-0 overflow-hidden mb-4" {
 div class="flex justify-between items-center" class="px-5 pt-5 pb-3" {
 span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" class="m-0 p-0 border-none" { "采购产品明细" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
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
 th class="w-[200px]" { "描述" }
 th class="w-[100px] text-right" { "数量" }
 th class="w-[120px] text-right" { "单价" }
 th class="w-[110px] text-right" { "小计" }
 th class="w-[80px] text-right" { "折扣%" }
 th class="w-[120px]" { "税率" }
 th class="w-[120px]" { "预期交货日期" }
 th class="w-9" { }
 }
 }
 tbody id="po-item-tbody" { }
 }
 }
 div class="p-3 flex items-center gap-2" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加产品行"
 }
 }
 div class="flex justify-end" class="p-4" class="border-t border-border" {
 div class="flex" class="text-sm" class="gap-6" {
 div { "不含税: " span id="sum-untaxed" class="font-semibold" { "0.00" } }
 div { "税额: " span id="sum-tax" class="font-semibold" { "0.00" } }
 div { "含税总计: " span id="sum-total" class="font-semibold" class="text-accent" { "0.00" } }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", POListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "保存草稿" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "提交订单"
 }
 }
 }
 script {
 (maud::PreEscaped("document.currentScript.parentElement.addEventListener('submit', function(ev){
 var errors=[];
 document.querySelectorAll('#po-item-tbody tr').forEach(function(row, i){
 var q=parseFloat(row.querySelector('[name=quantity]').value)||0;
 var p=parseFloat(row.querySelector('[name=unit_price]').value)||0;
 if(q<=0) errors.push('第'+(i+1)+'行数量必须大于0');
 if(p<=0) errors.push('第'+(i+1)+'行单价必须大于0');
 });
 if(errors.length>0){ alert(errors.join('\\n')); ev.preventDefault(); return; }
 var items=[];
 document.querySelectorAll('#po-item-tbody tr').forEach(function(row){
 var obj={};
 row.querySelectorAll('input,select,textarea').forEach(function(el){
 if(el.name && !obj[el.name]) obj[el.name]=el.value;
 });
 items.push(obj);
 });
 document.querySelector('#items-json').value=JSON.stringify(items);
 })"))
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", POItemRowPath::PATH, "po-item-tbody"))

 }
 }
}

/// Supplier detail fragment returned by HTMX on supplier select change
fn supplier_detail_fragment(contact_name: &str, contact_phone: &str, coop_years: i64) -> Markup {
 html! {
 div class="supplier-info-bar" class="flex bg-surface" class="rounded-sm" class="px-4 py-3 text-sm" class="gap-6 text-fg-2" {
 span { "联系人: " strong { (contact_name) } }
 span { "电话: " strong { (contact_phone) } }
 span { "地址: " strong { "—" } }
 span { "合作年限: " strong { (coop_years) " 年" } }
 }
 script {
 (maud::PreEscaped(format!("document.querySelector('#supplier-contact').value = '{}';", contact_name.replace('\'', "\\'"))))
 (maud::PreEscaped(format!("document.querySelector('#supplier-phone').value = '{}';", contact_phone.replace('\'', "\\'"))))
 }
 }
}

fn item_row_fragment(
 product: &abt_core::master_data::product::model::Product,
 tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
 let input_style = "width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)";
 html! {
 tr data-item-row="" {
 td class="text-muted text-xs text-center" { }
 td class="font-mono tabular-nums" { (product.product_code) }
 td { (product.pdt_name) }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="description" placeholder="—" class="text-[13px]" class="rounded-sm" class="px-2 py-[5px] border border-border" style="width:190px" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0.01" name="quantity" data-field="qty" placeholder="0" style=(input_style) {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" min="0.01" name="unit_price" data-field="price" placeholder="0.00" style=(input_style) {} }
 td class="line-subtotal font-mono tabular-nums" data-field="subtotal" class="text-right" { "0.00" }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="0.01" min="0" max="100" name="discount_pct" data-field="discount" value="0" placeholder="0" style=(input_style) {} }
 td {
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="tax_rate_id" data-field="tax_rate_id"
 class="w-[110px] text-[13px]" class="rounded-sm" class="px-2 py-[5px] border border-border" {
 option value="" { "—" }
 @for tr in tax_rates {
 option value=(tr.id) data-rate=(tr.rate.to_string()) { (tr.name) }
 }
 }
 }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="item_delivery_date" class="w-[110px] text-[13px]" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/> then call updatePurchaseSummary()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}
