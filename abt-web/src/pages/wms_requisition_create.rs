use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::Local;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::material_requisition::{CreateManualReq, CreateManualItemReq, MaterialRequisitionService};
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::*;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_requisition_create(
 _path: RequisitionCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
 .await?;

 let content = requisition_create_page(&warehouses.items);
 let page_html = admin_page(
 is_htmx,
 "新建领料单",
 &claims,
 "inventory",
 RequisitionCreatePath::PATH,
 "库存管理",
 Some("新建领料单"),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

/// HTMX: search products for the modal

/// HTMX: return a single item row fragment
#[require_permission("INVENTORY", "create")]
pub async fn get_item_row(
 ctx: RequestContext,
 Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.product_service();
 let product = svc.get(&service_ctx, &mut conn, params.product_id).await?;
 Ok(Html(item_row_fragment(&product).into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
struct RequisitionItemWeb {
 product_id: String,
 requested_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct RequisitionCreateForm {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub work_order_id: Option<i64>,
 #[serde(deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 pub requisition_date: String,
 pub items_json: String,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_requisition(
 _path: RequisitionCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<RequisitionCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.material_requisition_service();

 let requisition_date = chrono::NaiveDate::parse_from_str(&form.requisition_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("Invalid date: {e}")))?;

 let warehouse_id = form.warehouse_id
 .ok_or_else(|| DomainError::validation("Please select a warehouse"))?;

 // If work_order_id provided, use create_for_work_order
 if let Some(wo_id) = form.work_order_id
 && wo_id > 0 {
 let _id = svc.create_for_work_order(&service_ctx, &mut conn, wo_id).await
 .map_err(|e| {
 if matches!(e, DomainError::NotFound(_)) {
 DomainError::validation(format!("工单 {} 不存在", wo_id))
 } else {
 e
 }
 })?;
 let redirect = RequisitionListPath.to_string();
 return Ok(([("HX-Redirect", redirect)], Html(String::new())));
 }

 // Otherwise, manual create with items
 let web_items: Vec<RequisitionItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("Invalid item data: {e}")))?;

 let items: Vec<CreateManualItemReq> = web_items.into_iter().map(|it| {
 let product_id: i64 = it.product_id.parse().unwrap_or(0);
 let requested_qty: Decimal = it.requested_qty.parse().unwrap_or(Decimal::ZERO);
 CreateManualItemReq { product_id, requested_qty }
 }).collect();

 let req = CreateManualReq {
 warehouse_id,
 requisition_date,
 remark: None,
 items,
 };

 let _id = svc.create_manual(&service_ctx, &mut conn, req).await?;

 let redirect = RequisitionListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn requisition_create_page(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
 div {
 // ── Back Link ──
 a href=(format!("{}?restore=true", RequisitionListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回领料单列表"
 }
 // ── Page Header ──
 div class="flex items-center justify-between mb-5" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建领料单" }
 span class="text-xs text-muted flex items-center gap-2" {
 (icon::clock_icon("w-3.5 h-3.5"))
 "自动保存草稿"
 }
 }
 form hx-post=(RequisitionCreatePath::PATH) hx-swap="none" id="requisitionForm"
 onsubmit="return reqCollectItems()" {
 // ── 工单信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::clipboard_document_icon("w-[18px] h-[18px]"))
 "工单信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工单" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" name="work_order_id" placeholder="输入工单号（留空为手动创建）";
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "领料仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="warehouse_id" required {
 option value="" { "请选择仓库" }
 @for w in warehouses {
 option value=(w.id) { (w.name) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "领料日期 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="requisition_date" required value=(Local::now().format("%Y-%m-%d")) {}
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "操作员" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm text-fg outline-none transition-all duration-150 focus:border-accent" type="text" readonly class="bg-surface" value="admin";
 }
 }
 }
 // ── 领料明细 ──
 div class="form-section" class="p-0 overflow-hidden" {
 div class="px-6 pt-6 pb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "领料明细"
 span id="req-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10 text-center" { "行号" }
 th style="min-width:130px" { "产品编码" }
 th style="min-width:180px" { "产品名称" }
 th class="min-w-[160px]" { "规格" }
 th class="w-16" { "单位" }
 th class="w-[110px] text-right" { "请求数量 " span class="required" { "*" } }
 th class="w-10" { }
 }
 }
 tbody id="req-item-tbody" { }
 }
 }
 div class="p-4" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-[#2563eb] text-sm font-medium cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加物料"
 }
 }
 }
 input type="hidden" name="items_json" id="req-items-json" value="[]" {}
 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 div { }
 div class="flex gap-3" {
 a href=(format!("{}?restore=true", RequisitionListPath::PATH)) class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "提交领料单"
 }
 }
 }
 }
 (crate::components::product_picker::product_picker_modal_with_search("product-modal", RequisitionItemRowPath::PATH, "req-item-tbody"))
 // ── JS ──
 (maud::PreEscaped(r#"<script>
 function reqCalcSummary() {
 var tbody = document.getElementById('req-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 document.getElementById('req-item-count').textContent = '共 ' + rows.length + ' 项';
 }
 function reqRenumber() {
 var tbody = document.getElementById('req-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 rows.forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 reqCalcSummary();
 }
 function reqCollectItems() {
 var tbody = document.getElementById('req-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 requested_qty: row.querySelector('input[name="requested_qty"]').value || '0'
 });
 });
 document.getElementById('req-items-json').value = JSON.stringify(items);
 if (items.length === 0) {
 alert('请至少添加一个物料');
 return false;
 }
 return true;
 }
 </script>"#))
 }
}
}

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
 tr {
 td class="text-muted text-xs text-center line-num" { }
 td class="font-mono tabular-nums" { (product.product_code) }
 td { (product.pdt_name) }
 td class="text-sm text-fg-2" { (product.meta.specification) }
 td class="text-sm text-fg-2 text-center" { (product.unit) }
 td {
 input class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="number" min="0.01" step="any" name="requested_qty" placeholder="0" {}
 }
 td {
 button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 _="on click remove closest <tr/> then call reqRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 }
 }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}
