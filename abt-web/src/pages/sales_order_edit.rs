use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::{CustomerContact, CustomerQuery};
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::types::PageParams;

use crate::components::customer_info::customer_info_panel;
use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct OrderEditForm {
 pub customer_id: i64,
 pub contact_id: i64,
 pub payment_terms: Option<String>,
 pub delivery_terms: Option<String>,
 pub delivery_address: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
 product_id: String,
 description: Option<String>,
 quantity: String,
 unit: Option<String>,
 unit_price: String,
 unit_cost: Option<String>,
 discount_rate: Option<String>,
 item_delivery_date: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_edit(
 path: OrderEditFormPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();
 let customer_svc = state.customer_service();
 let product_svc = state.product_service();

 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

 let items = svc.list_items(&service_ctx, &mut conn, path.id).await?;

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, order.customer_id).await.unwrap_or_default();

 // Resolve product codes for items
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 let product_codes: std::collections::HashMap<i64, (String, String)> = if !product_ids.is_empty() {
 let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
 products.into_iter().map(|p| (p.product_id, (p.product_code, p.pdt_name))).collect()
 } else {
 std::collections::HashMap::new()
 };

 let content = order_edit_page(&order, &items, &customers.items, &contacts, &product_codes);
 let page_html = admin_page(
 is_htmx, "编辑订单", &claims, "sales", OrderEditFormPath::PATH, "销售管理", Some("编辑订单"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

/// POST: update order
#[require_permission("SALES_ORDER", "update")]
pub async fn update_order(
 path: OrderEditFormPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<OrderEditForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 if form.customer_id == 0 {
 return Err(DomainError::validation("请选择客户").into());
 }
 if form.contact_id == 0 {
 return Err(DomainError::validation("请选择联系人").into());
 }

 let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

 if web_items.is_empty() {
 return Err(DomainError::validation("请至少添加一个产品").into());
 }

 let items: Vec<CreateSalesOrderItemReq> = web_items.into_iter().map(|item| {
 CreateSalesOrderItemReq {
 product_id: item.product_id.parse().unwrap_or(0),
 description: item.description,
 quantity: item.quantity.parse().unwrap_or(rust_decimal::Decimal::ONE),
 unit: item.unit,
 unit_price: item.unit_price.parse().unwrap_or(rust_decimal::Decimal::ZERO),
 unit_cost: item.unit_cost.and_then(|s| s.parse().ok()),
 discount_rate: item.discount_rate.and_then(|s| s.parse().ok()),
 delivery_date: item.item_delivery_date.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
 }
 }).collect();

 let total: rust_decimal::Decimal = items.iter().map(|i| {
 let subtotal = i.quantity * i.unit_price;
 let discount = i.discount_rate.unwrap_or(rust_decimal::Decimal::ZERO) / rust_decimal::Decimal::ONE_HUNDRED;
 subtotal * (rust_decimal::Decimal::ONE - discount)
 }).sum();
 if total <= rust_decimal::Decimal::ZERO {
 return Err(DomainError::validation("订单总额不能为零，请填写产品单价").into());
 }

 let req = UpdateSalesOrderReq {
 customer_id: Some(form.customer_id),
 contact_id: Some(form.contact_id),
 payment_terms: form.payment_terms,
 delivery_terms: form.delivery_terms,
 delivery_address: form.delivery_address,
 remark: form.remark,
 };

 svc.update(&service_ctx, &mut conn, path.id, req, items).await?;

 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn order_edit_page(
 order: &SalesOrder,
 items: &[SalesOrderItem],
 customers: &[abt_core::master_data::customer::model::Customer],
 contacts: &[CustomerContact],
 product_codes: &std::collections::HashMap<i64, (String, String)>,
) -> Markup {

 let detail_path = OrderDetailPath { id: order.id };
 let update_path = OrderEditFormPath { id: order.id };

 // Pre-select payment/delivery terms
 let pt = &order.payment_terms;
 let dt = &order.delivery_terms;
 let da = &order.delivery_address;
 let rm = &order.remark;

 html! {
 div id="order-app" {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(detail_path.to_string()) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回订单详情"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "编辑订单 " (order.doc_number) }
 }

 form id="order-form"
 hx-post=(update_path.to_string())
 hx-swap="none"
 onsubmit="lineItemCalc('#order-item-tbody').collectItems()" {
 input type="hidden" id="items-json" name="items_json" value="[]";

 // ── Customer Info ──
 (customer_info_panel(customers, contacts, Some(order.customer_id), OrderCustomerContactsPath::PATH))

 // ── Order Info ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "订单信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "订单日期" }
 input type="date" value=(order.order_date.format("%Y-%m-%d")) disabled {}
 }
 div class="form-field" {
 label { "付款条款" }
 select name="payment_terms" {
 option value="30天净额" selected[*pt == "30天净额"] { "30天净额" }
 option value="60天净额" selected[*pt == "60天净额"] { "60天净额" }
 option value="预付30%" selected[*pt == "预付30%"] { "预付30%" }
 option value="货到付款" selected[*pt == "货到付款"] { "货到付款" }
 option value="月结30天" selected[*pt == "月结30天"] { "月结30天" }
 }
 }
 div class="form-field" {
 label { "交货条款" }
 select name="delivery_terms" {
 option value="FOB 深圳" selected[*dt == "FOB 深圳"] { "FOB 深圳" }
 option value="FOB 广州" selected[*dt == "FOB 广州"] { "FOB 广州" }
 option value="CIF 目的港" selected[*dt == "CIF 目的港"] { "CIF 目的港" }
 option value="EXW 工厂交货" selected[*dt == "EXW 工厂交货"] { "EXW 工厂交货" }
 }
 }
 div class="form-field" {
 label { "交货地址" }
 input type="text" name="delivery_address" value=(da) {}
 }
 }
 }

 // ── Line Items ──
 div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden flush mb-4" {
 div class="flush-header" {
 span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "产品明细" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加产品"
 }
 }
 div class="flush-scroll" {
 table class="data-table" {
 thead {
 tr {
 th class="w-9" { "#" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格描述" }
 th class="w-14" { "单位" }
 th class="w-[90px]" { "数量" }
 th class="w-[110px]" { "单价 (¥)" }
 th style="width:76px" { "折扣%" }
 th class="w-[110px]" { "小计 (¥)" }
 th class="w-[110px]" { "交货日期" }
 th class="w-9" { }
 }
 }
 tbody id="order-item-tbody" {
 @for item in items {
 @let (code, name) = product_codes.get(&item.product_id).cloned().unwrap_or_default();
 tr {
 td class="text-muted text-xs text-center" { }
 td class="font-mono tabular-nums" { (code) }
 td { (name) }
 td { input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm outline-none focus:border-accent" type="text" name="description" value=(&item.description) {} }
 td { input class="w-[56px] text-center px-2 py-[5px] text-[13px] border border-border rounded-sm bg-surface outline-none focus:border-accent" type="text" name="unit" readonly value=(&item.unit) {} }
 td { input class="w-[80px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent" type="number" min="1" step="1" name="quantity" value=(item.quantity.to_string()) placeholder="0" {} }
 td { input class="w-[100px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent" type="number" step="any" name="unit_price" value=(item.unit_price.to_string()) placeholder="0.00" {} }
 td { input class="w-[64px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent" type="number" min="0" max="100" name="discount_rate" value=(item.discount_rate.to_string()) {} }
 td class="text-right font-semibold text-fg whitespace-nowrap" { "—" }
 td { input class="w-[110px] px-1.5 py-[5px] text-xs border border-border rounded-sm outline-none focus:border-accent" type="date" name="item_delivery_date" value=(item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()) {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/>" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(item.product_id) {}
 }
 }
 }
 }
 }
 div class="p-3 flex items-center gap-2" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加产品行"
 }
 }
 div class="flex justify-end p-4 bg-surface border-t border-border-soft gap-8" {
 div class="flex gap-3" {
 span class="text-sm text-muted" { "合计金额" }
 span class="text-lg font-bold text-fg" id="subtotal-value" { "¥ 0.00" }
 }
 div class="flex gap-3" {
 span class="text-sm text-muted" { "折扣总额" }
 span class="text-lg font-bold text-fg" id="discount-value" { "- ¥ 0.00" }
 }
 div class="flex gap-3" {
 span class="text-sm text-muted" { "订单总额" }
 span class="text-lg font-bold text-fg grand" id="grand-value" { "¥ 0.00" }
 }
 }
 }

 // ── Remark ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "备注" }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] min-h-[72px] resize-y leading-1.5" name="remark" placeholder="输入订单相关备注信息…" { (rm) }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(detail_path.to_string()) { "取消" }
 div class="flex gap-3" {
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "保存修改"
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", OrderItemRowPath::PATH, "order-item-tbody"))

 (maud::PreEscaped(r#"<script>
 function oeRecalc() { lineItemCalc('#order-item-tbody').recalcTotals() }
 document.addEventListener('input', function(e) {
 if (e.target.classList.contains('num-input')) {
 var row = e.target.closest('tr');
 if (row && row.closest('#order-item-tbody')) oeRecalc();
 }
 });
 document.addEventListener('htmx:afterSettle', function(e) {
 if (e.target.querySelector && e.target.querySelector('#order-item-tbody')) oeRecalc();
 });
 document.addEventListener('DOMContentLoaded', oeRecalc);
 </script>"#))
 }
 }
}
