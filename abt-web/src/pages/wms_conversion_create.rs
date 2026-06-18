use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::form_conversion::FormConversionService;
use abt_core::wms::form_conversion::model::{CreateConversionReq, CreateConversionItemReq};
use abt_core::wms::enums::ConversionDir;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_conversion::*;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_conversion_create(
 _path: ConversionCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = conversion_create_page(&warehouses);
 let page_html = admin_page(
 is_htmx,
 "新建形态转换单",
 &claims,
 "inventory",
 ConversionCreatePath::PATH,
 "库存管理",
 None,
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
struct ConversionItemWeb {
 product_id: String,
 quantity: String,
 unit_cost: Option<String>,
 batch_no: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConversionCreateForm {
 #[serde(deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 pub conversion_date: String,
 pub remark: Option<String>,
 pub consume_json: String,
 pub produce_json: String,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_conversion(
 _path: ConversionCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ConversionCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.form_conversion_service();

 let consume_items: Vec<ConversionItemWeb> = serde_json::from_str(&form.consume_json)
 .map_err(|e| DomainError::validation(format!("消耗物料数据无效: {e}")))?;
 let produce_items: Vec<ConversionItemWeb> = serde_json::from_str(&form.produce_json)
 .map_err(|e| DomainError::validation(format!("产出物料数据无效: {e}")))?;

 if consume_items.is_empty() && produce_items.is_empty() {
 return Err(DomainError::validation("请至少添加一行消耗物料或产出物料").into());
 }

 let mut items: Vec<CreateConversionItemReq> = consume_items.into_iter().map(|it| {
 let product_id: i64 = it.product_id.parse().unwrap_or(0);
 let quantity: Decimal = it.quantity.parse().unwrap_or(Decimal::ZERO);
 let unit_cost: Decimal = it.unit_cost.as_ref().and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
 CreateConversionItemReq {
 direction: ConversionDir::Consume,
 product_id,
 quantity,
 unit_cost,
 batch_no: it.batch_no.filter(|s| !s.is_empty()),
 }
 }).collect();

 items.extend(produce_items.into_iter().map(|it| {
 let product_id: i64 = it.product_id.parse().unwrap_or(0);
 let quantity: Decimal = it.quantity.parse().unwrap_or(Decimal::ZERO);
 let unit_cost: Decimal = it.unit_cost.as_ref().and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
 CreateConversionItemReq {
 direction: ConversionDir::Produce,
 product_id,
 quantity,
 unit_cost,
 batch_no: it.batch_no.filter(|s| !s.is_empty()),
 }
 }));

 let conversion_date = chrono::NaiveDate::parse_from_str(&form.conversion_date, "%Y-%m-%d")
 .map_err(|_| DomainError::validation("无效的转换日期格式"))?;

 let warehouse_id = form.warehouse_id
 .ok_or_else(|| DomainError::validation("请选择转换仓库"))?;

 let req = CreateConversionReq {
 warehouse_id,
 conversion_date,
 remark: form.remark.filter(|s| !s.is_empty()).unwrap_or_default(),
 items,
 };

 let _id = svc.create(&service_ctx, &mut conn, req).await?;

 let redirect = ConversionListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn conversion_create_page(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
 div {
 // ── Back Link ──
 a href=(format!("{}?restore=true", ConversionListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回形态转换列表"
 }
 // ── Page Header ──
 div class="flex items-center justify-between mb-5" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建形态转换单" }
 span class="text-xs text-muted flex items-center gap-2" {
 (icon::clock_icon("w-3.5 h-3.5"))
 "自动保存草稿"
 }
 }
 form hx-post=(ConversionCreatePath::PATH) hx-swap="none" id="conversionForm"
 onsubmit="return conversionCollectItems()" {
 // ── 转换信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::refresh_icon("w-[18px] h-[18px]"))
 "转换信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="warehouse_id" required {
 option value="" { "请选择仓库" }
 @for w in warehouses {
 option value=(w.id) { (w.name) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "转换日期 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="conversion_date" required {}
 }
 div class="form-field col-span-2" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="remark" {}
 }
 }
 }
 // ── 消耗物料 ──
 div class="form-section p-0 overflow-hidden" {
 div class="px-6 pt-6 pb-4 flex items-center gap-2" {
 span class="text-sm font-semibold text-fg" { "消耗物料" }
 span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-[#fff2f0] text-danger" { "消耗" }
 span id="consume-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10 text-center" { "行号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格" }
 th class="w-[100px] text-right" { "数量 " span class="required" { "*" } }
 th class="w-[110px] text-right" { "单位成本" }
 th class="w-[120px]" { "批次号" }
 th class="w-10" { }
 }
 }
 tbody id="consume-item-tbody" { }
 }
 }
 div class="p-4" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-[#2563eb] text-sm font-medium cursor-pointer"
 onclick="conversionOpenModal('consume')" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加消耗行"
 }
 }
 }
 // ── 产出物料 ──
 div class="form-section p-0 overflow-hidden" {
 div class="px-6 pt-6 pb-4 flex items-center gap-2" {
 span class="text-sm font-semibold text-fg" { "产出物料" }
 span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-[#f0fff0] text-success" { "产出" }
 span id="produce-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10 text-center" { "行号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格" }
 th class="w-[100px] text-right" { "数量 " span class="required" { "*" } }
 th class="w-[110px] text-right" { "单位成本" }
 th class="w-[120px]" { "批次号" }
 th class="w-10" { }
 }
 }
 tbody id="produce-item-tbody" { }
 }
 }
 div class="p-4" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-[#2563eb] text-sm font-medium cursor-pointer"
 onclick="conversionOpenModal('produce')" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加产出行"
 }
 }
 }
 input type="hidden" name="consume_json" id="consume-json" value="[]" {}
 input type="hidden" name="produce_json" id="produce-json" value="[]" {}
 div id="conversion-item-target" class="hidden" { }
 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 div { }
 div class="flex gap-3" {
 a href=(format!("{}?restore=true", ConversionListPath::PATH)) class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "提交" }
 }
 }
 }
 (crate::components::product_picker::product_picker_modal_with_search("product-modal", ConversionItemRowPath::PATH, "conversion-item-tbody"))
 // ── JS ──
 (maud::PreEscaped(r#"<script>
 var conversionTarget = 'consume';
 function conversionOpenModal(target) {
 conversionTarget = target;
 document.querySelector('#product-modal').classList.add('is-open');
 }
 function conversionRenumber(tbodyId) {
 var tbody = document.getElementById(tbodyId);
 var rows = tbody.querySelectorAll('tr');
 rows.forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 var countId = tbodyId === 'consume-item-tbody' ? 'consume-item-count' : 'produce-item-count';
 document.getElementById(countId).textContent = '共 ' + rows.length + ' 项';
 }
 function conversionCollectItems() {
 var consumeTbody = document.getElementById('consume-item-tbody');
 var produceTbody = document.getElementById('produce-item-tbody');
 var consumeItems = [];
 var produceItems = [];
 consumeTbody.querySelectorAll('tr').forEach(function(row) {
 consumeItems.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 quantity: row.querySelector('input[name="quantity"]').value || '0',
 unit_cost: row.querySelector('input[name="unit_cost"]').value || null,
 batch_no: row.querySelector('input[name="batch_no"]').value || null
 });
 });
 produceTbody.querySelectorAll('tr').forEach(function(row) {
 produceItems.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 quantity: row.querySelector('input[name="quantity"]').value || '0',
 unit_cost: row.querySelector('input[name="unit_cost"]').value || null,
 batch_no: row.querySelector('input[name="batch_no"]').value || null
 });
 });
 document.getElementById('consume-json').value = JSON.stringify(consumeItems);
 document.getElementById('produce-json').value = JSON.stringify(produceItems);
 if (consumeItems.length === 0 && produceItems.length === 0) {
 alert('请至少添加一行消耗物料或产出物料');
 return false;
 }
 return true;
 }
 function conversionAfterAdd() {
 var target = document.getElementById('conversion-item-target');
 var tbodyId = conversionTarget === 'consume' ? 'consume-item-tbody' : 'produce-item-tbody';
 var tbody = document.getElementById(tbodyId);
 while (target.firstChild) {
 tbody.appendChild(target.firstChild);
 }
 document.querySelector('#product-modal').classList.remove('is-open');
 conversionRenumber(tbodyId);
 }
 function conversionRemoveRow(btn) {
 btn.closest('tr').remove();
 conversionRenumber('consume-item-tbody');
 conversionRenumber('produce-item-tbody');
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
 td {
 input class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="number" min="0.01" step="any" name="quantity" placeholder="0" {}
 }
 td {
 input class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="number" step="any" name="unit_cost" placeholder="0.00" {}
 }
 td {
 input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="text" name="batch_no" placeholder="批次号" {}
 }
 td {
 button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 onclick="conversionRemoveRow(this)" {
 (icon::x_icon("w-3.5 h-3.5"))
 }
 }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
}
}
