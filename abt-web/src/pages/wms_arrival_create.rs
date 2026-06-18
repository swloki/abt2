use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::Local;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::master_data::supplier::SupplierStatus;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::arrival_notice::{ArrivalNoticeService, CreateArrivalNoticeItemReq, CreateArrivalNoticeReq};
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_arrival::*;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_arrival_create(
 _path: ArrivalCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let supplier_svc = state.supplier_service();
 let warehouse_svc = state.warehouse_service();

 let suppliers = supplier_svc
 .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
 .await?;

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
 .await?;

 let content = arrival_create_page(&suppliers.items, &warehouses.items, &claims.display_name);
 let page_html = admin_page(
 is_htmx,
 "新建来料通知",
 &claims,
 "inventory",
 ArrivalCreatePath::PATH,
 "库存管理",
 Some("新建来料通知"),
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

// ── PO Import Handlers ──

#[derive(Debug, Deserialize)]
pub struct PoSearchParams {
 pub keyword: Option<String>,
}

/// HTMX: search confirmed POs for import
#[require_permission("INVENTORY", "create")]
pub async fn get_po_pick(
 ctx: RequestContext,
 Query(params): Query<PoSearchParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();
 let supplier_svc = state.supplier_service();

 let query = PurchaseOrderQuery::default();
 let kw = params.keyword.as_deref().unwrap_or("").trim().to_lowercase();

 let orders = svc
 .list(&service_ctx, &mut conn, query, PageParams::new(1, 50))
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 // 过滤：只显示 Confirmed 状态 + 关键词匹配 doc_number
 let filtered: Vec<_> = orders.into_iter()
 .filter(|o| o.status == PurchaseOrderStatus::Confirmed)
 .filter(|o| kw.is_empty() || o.doc_number.to_lowercase().contains(&kw))
 .collect();

 // 批量获取供应商名
 let supplier_ids: Vec<i64> = filtered.iter().map(|o| o.supplier_id).collect();
 let suppliers = supplier_svc
 .list(&service_ctx, &mut conn, SupplierQuery::default(), PageParams::new(1, 200))
 .await
 .map(|r| r.items)
 .unwrap_or_default();
 let supplier_map: std::collections::HashMap<i64, String> = suppliers
 .into_iter()
 .map(|s| (s.id, s.name))
 .collect();

 Ok(Html(po_list_fragment(&filtered, &supplier_map).into_string()))
}

/// HTMX: return PO items as arrival item rows
#[require_permission("INVENTORY", "create")]
pub async fn get_po_items(
 path: ArrivalPoItemsPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let po_svc = state.purchase_order_service();
 let product_svc = state.product_service();

 let items = po_svc.list_items(&service_ctx, &mut conn, path.po_id).await?;
 let po = po_svc.get(&service_ctx, &mut conn, path.po_id).await?;

 // 批量获取产品信息
 let mut product_map = std::collections::HashMap::new();
 for item in &items {
 if !product_map.contains_key(&item.product_id) {
 if let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
 product_map.insert(item.product_id, p);
 }
 }
 }

 Ok(Html(po_items_fragment(&items, &product_map, po.supplier_id).into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ArrivalCreateForm {
 #[serde(deserialize_with = "empty_as_none")]
 pub purchase_order_id: Option<i64>,
 #[serde(deserialize_with = "empty_as_none")]
 pub supplier_id: Option<i64>,
 pub arrival_date: String,
 #[serde(deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
 pub delivery_note: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ArrivalItemWeb {
 product_id: String,
 declared_qty: String,
 batch_no: Option<String>,
 #[serde(default)]
 order_item_id: Option<String>,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_arrival(
 _path: ArrivalCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ArrivalCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.arrival_notice_service();

 let arrival_date = chrono::NaiveDate::parse_from_str(&form.arrival_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效的到货日期: {e}")))?;

 let web_items: Vec<ArrivalItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("物料明细数据无效: {e}")))?;

 if web_items.is_empty() {
 return Err(DomainError::validation("请添加至少一条物料明细").into());
 }

 let items: Vec<CreateArrivalNoticeItemReq> = web_items.into_iter().map(|it| {
 let product_id: i64 = it.product_id.parse()
 .map_err(|_| DomainError::validation("无效产品ID")).unwrap_or(0);
 let declared_qty: Decimal = it.declared_qty.parse()
 .map_err(|_| DomainError::validation("无效数量")).unwrap_or(Decimal::ZERO);
 let order_item_id = it.order_item_id
 .filter(|s| !s.is_empty())
 .and_then(|s| s.parse::<i64>().ok());
 CreateArrivalNoticeItemReq {
 order_item_id,
 product_id,
 declared_qty,
 batch_no: it.batch_no.filter(|s| !s.is_empty()),
 }
 }).collect();

 let req = CreateArrivalNoticeReq {
 purchase_order_id: form.purchase_order_id,
 supplier_id: form.supplier_id.ok_or_else(|| {
 DomainError::validation("请选择供应商")
 })?,
 arrival_date,
 warehouse_id: form.warehouse_id.ok_or_else(|| DomainError::validation("请选择仓库"))?,
 zone_id: form.zone_id,
 delivery_note: form.delivery_note.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()).unwrap_or_default(),
 items,
 };

 let id = svc.create(&service_ctx, &mut conn, req).await?;

 let redirect = format!("{}/{}", ArrivalListPath::PATH, id);
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn arrival_create_page(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 operator_name: &str,
) -> Markup {
 html! {
 div {
 a href=(format!("{}?restore=true", ArrivalListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回来料通知列表"
 }

 div class="flex items-center justify-between mb-5" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建来料通知" }
 div class="flex gap-3" {
 span class="flex items-center gap-2 text-xs text-muted" {
 (icon::clock_icon("w-3.5 h-3.5"))
 "操作员: " (operator_name)
 }
 }
 }

 form class="space-y-5" hx-post=(ArrivalCreatePath::PATH) hx-swap="none" id="arrivalForm"
 onsubmit="return arrivalCollectItems()" {
 // ── 供应商信息 ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::building_icon("w-4 h-4"))
 "供应商信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商 " span class="text-danger" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="supplier-select" name="supplier_id" required {
 option value="" { "请选择供应商" }
 @for s in suppliers {
 option value=(s.id) { (s.name) }
 }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系人" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly tabindex="-1" placeholder="自动填充";
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系电话" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly tabindex="-1" placeholder="自动填充";
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源采购单" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="purchase_order_id" {
 option value="" { "请选择采购单（可选）" }
 }
 }
 }
 }

 // ── 到货信息 ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::truck_icon("w-4 h-4"))
 "到货信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "到货仓库 " span class="text-danger" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" required {
 option value="" { "请选择仓库" }
 @for w in warehouses {
 option value=(w.id) { (w.name) }
 }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "到货库区" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" {
 option value="" { "请选择库区" }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "到货日期 " span class="text-danger" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="arrival_date" required value=(Local::now().format("%Y-%m-%d"));
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "送货单号" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="delivery_note" placeholder="请输入送货单号";
 }
 }
 }

 // ── 物料明细 ──
 div class="bg-bg border border-border rounded-md overflow-hidden" {
 div class="px-6 pt-6 pb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg pb-2 [border-bottom:1px_solid_var(--border-soft)]" {
 (icon::box_icon("w-4 h-4"))
 "物料明细"
 span id="arrival-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10 text-center" { "行号" }
 th class="min-w-[140px]" { "产品编码" }
 th class="min-w-[200px]" { "产品名称" }
 th class="min-w-[160px]" { "规格" }
 th class="w-[100px] text-right" { "申报数量 " span class="text-danger" { "*" } }
 th class="w-[140px]" { "批次号" }
 th class="w-10" { "操作" }
 }
 }
 tbody id="arrival-item-tbody" {
 // JS-managed dynamic rows
 }
 }
 }
 div class="p-3 flex items-center gap-2" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-4 h-4"))
 "添加物料"
 }
 button type="button" class="inline-flex items-center gap-2 ml-3 rounded-sm text-accent text-sm cursor-pointer"
 _="on click add .is-open to #po-modal" {
 (icon::download_icon("w-4 h-4"))
 "从采购订单导入"
 }
 }
 }

 // ── 备注 ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::edit_icon("w-4 h-4"))
 "备注"
 }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[80px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" rows="3" placeholder="请输入备注信息" {}
 }

 input type="hidden" name="purchase_order_id" id="arrival-po-id" value="" {}
 input type="hidden" name="items_json" id="arrival-items-json" value="[]" {}

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 a href=(format!("{}?restore=true", ArrivalListPath::PATH)) class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "提交来料通知"
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", ArrivalItemRowPath::PATH, "arrival-item-tbody"))

 // ── PO Import Modal ──
 div id="po-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
 _="on click[me is event.target] remove .is-open" {
 div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" _="on click halt the event" {
 div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
 h2 { "从采购订单导入" }
 button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
 _="on click remove .is-open from #po-modal" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-4" hx-disinherit="hx-select" {
 div class="flex gap-4 p-4 [border-bottom:1px_solid_var(--border-soft)]" {
 div class="flex-1 flex flex-col gap-[4px]" {
 label class="text-[12px] font-medium text-fg-2" { "采购订单号" }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" id="po-search-input" name="keyword" placeholder="输入PO编号搜索…"
 hx-get=(ArrivalPoPickPath::PATH)
 hx-trigger="keyup changed delay:300ms"
 hx-sync="this:replace"
 hx-target="#po-search-results"
 hx-swap="innerHTML"
 hx-include="#po-search-input" {}
 }
 button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
 hx-get=(ArrivalPoPickPath::PATH)
 hx-target="#po-search-results"
 hx-swap="innerHTML"
 _="on click set #po-search-input's value to '' then trigger keyup on #po-search-input" {
 "清除"
 }
 }
 div id="po-search-results" {
 div class="text-center text-muted py-12" {
 (icon::package_icon("w-8 h-8"))
 p class="mt-2 text-sm" { "输入PO编号搜索已确认的采购订单" }
 }
 }
 }
 }
 }

 // ── JS ──
 (maud::PreEscaped(r#"<script>
 function arrivalCalcSummary() {
 var tbody = document.getElementById('arrival-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 document.getElementById('arrival-item-count').textContent = '共 ' + rows.length + ' 项';
 }

 function arrivalCollectItems() {
 var tbody = document.getElementById('arrival-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 var oiInput = row.querySelector('input[name="order_item_id"]');
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 declared_qty: row.querySelector('input[name="declared_qty"]').value || '0',
 batch_no: row.querySelector('input[name="batch_no"]').value || null,
 order_item_id: oiInput ? oiInput.value : null
 });
 });
 document.getElementById('arrival-items-json').value = JSON.stringify(items);
 if (items.length === 0) {
 alert('请至少添加一个物料');
 return false;
 }
 return true;
 }

 function arrivalRenumber() {
 var tbody = document.getElementById('arrival-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 rows.forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 arrivalCalcSummary();
 }
 </script>"#))
 }
}

const CELL_INPUT: &str =
 "w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg \
  outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]";

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 arrival_line_row(product, "", None)
}

/// PO items rendered as arrival item rows (appended to tbody)
fn po_items_fragment(
 items: &[abt_core::purchase::order::model::PurchaseOrderItem],
 product_map: &std::collections::HashMap<i64, abt_core::master_data::product::model::Product>,
 _supplier_id: i64,
) -> Markup {
 html! {
 @for item in items {
 @if let Some(product) = product_map.get(&item.product_id) {
 (arrival_line_row(product, &item.quantity.to_string(), Some(item.id)))
 }
 }
 }
}

fn arrival_line_row(product: &abt_core::master_data::product::model::Product, qty: &str, order_item_id: Option<i64>) -> Markup {
 html! {
 tr {
 td class="line-num text-muted text-xs text-center" { }
 td class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
 td class="text-sm text-fg" { (product.pdt_name) }
 td class="text-sm text-fg-2" { (product.meta.specification) }
 td { input class=(format!("{CELL_INPUT} w-[90px] text-right font-mono")) type="number" min="0.01" step="any" name="declared_qty" placeholder="0" value=(qty) {} }
 td { input class=(format!("{CELL_INPUT} w-[120px]")) type="text" name="batch_no" placeholder="批次号" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 _="on click remove closest <tr/> then call arrivalRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 @if let Some(oi) = order_item_id {
 input type="hidden" name="order_item_id" value=(oi) {}
 }
 }
 }
}

/// PO search results fragment for import modal
fn po_list_fragment(
 orders: &[abt_core::purchase::order::model::PurchaseOrder],
 supplier_map: &std::collections::HashMap<i64, String>,
) -> Markup {
 html! {
 @if orders.is_empty() {
 div class="text-center text-muted py-12" {
 (icon::package_icon("w-8 h-8"))
 p class="mt-2 text-sm" { "未找到已确认的采购订单" }
 }
 } @else {
 div class="py-2" {
 @for o in orders {
div class="flex items-center justify-between p-3 [border-bottom:1px_solid_var(--border-soft)]" {
 div class="flex-1 min-w-0" {
 div class="text-sm font-medium text-fg" { (o.doc_number) }
 div class="text-[12px] text-muted flex items-center gap-[6px] flex-wrap" {
 span class="bg-surface rounded-sm" { (supplier_map.get(&o.supplier_id).cloned().unwrap_or_else(|| "-".into())) }
 span class="text-border" { "·" }
 span { (o.order_date.format("%Y-%m-%d")) }
 span class="text-border" { "·" }
 span { "¥" (o.total_amount) }
 }
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_svg]:w-4 [&_svg]:h-4"
 hx-get=(ArrivalPoItemsPath { po_id: o.id }.to_string())
 hx-target="#arrival-item-tbody"
 hx-swap="beforeend"
 _=(format!("on click set #arrival-po-id's value to '{}' then set #supplier-select's value to '{}' then remove .is-open from #po-modal end on htmx:afterRequest[detail.xhr.status < 400] wait 50ms then call arrivalRenumber()", o.id, o.supplier_id)) {
 "导入"
 }
 }
 }
 }
 }
 }
}

