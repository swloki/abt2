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
 a href=(format!("{}?restore=true", RequisitionListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回领料单列表"
 }

 div class="flex items-center justify-between mb-6" style="margin-bottom:var(--space-5)" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建领料单" }
 }

 div class="flex items-center" {
 div class="flex items-center gap-2 text-xs text-muted current" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "草稿" }
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 text-xs text-muted" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "已确认" }
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 text-xs text-muted" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "已发料" }
 }

 form hx-post=(RequisitionCreatePath::PATH) hx-swap="none" id="requisitionForm"
 onsubmit="return reqCollectItems()" {
 // Basic info
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::clipboard_document_icon("w-4 h-4"))
 "领料信息"
 }
 div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "领料仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" required {
 option value="" { "请选择仓库" }
 @for w in warehouses {
 option value=(w.id) { (w.name) }
 }
 }
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "领料日期 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="requisition_date" required value=(Local::now().format("%Y-%m-%d")) {}
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工单（可选）" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="work_order_id" placeholder="留空为手动创建";
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "操作员" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly style="background:var(--surface)" value="admin";
 }
 }
 }

 // Line items
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::box_icon("w-4 h-4"))
 "领料明细"
 span id="req-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
 }
 div style="overflow-x:auto" {
 table class="data-table" {
 thead {
 tr {
 th style="width:40px;text-align:center" { "行号" }
 th style="min-width:140px" { "产品编码" }
 th style="min-width:200px" { "产品名称" }
 th style="min-width:160px" { "规格" }
 th style="width:110px;text-align:right" { "请求数量 " span class="required" { "*" } }
 th style="width:40px" { }
 }
 }
 tbody id="req-item-tbody" { }
 }
 }
 div class="p-3 flex items-center gap-2" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-4 h-4"))
 "添加物料"
 }
 }
 }

 input type="hidden" name="items_json" id="req-items-json" value="[]" {}

 // Actions
 div class="action-bar" {
 a href=(format!("{}?restore=true", RequisitionListPath::PATH)) class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "提交领料单"
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", RequisitionItemRowPath::PATH, "requisition-item-tbody"))

 // JS
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

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
 tr {
 td class="text-muted text-xs text-center" { }
 td class="font-mono tabular-nums" { (product.product_code) }
 td { (product.pdt_name) }
 td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0.01" step="any" name="requested_qty" placeholder="0" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/> then call reqRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}
