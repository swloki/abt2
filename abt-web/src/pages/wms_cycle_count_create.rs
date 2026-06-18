use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::cycle_count::model::{CreateCycleCountReq, CreateCycleCountItemReq};
use abt_core::wms::cycle_count::CycleCountService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_cycle_count::*;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
struct CycleCountItemWeb {
 product_id: String,
 bin_id: Option<String>,
 batch_no: Option<String>,
 system_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCycleCountForm {
 #[serde(deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
 pub count_date: String,
 pub is_blind: Option<String>,
 pub remark: Option<String>,
 pub action: Option<String>,
 pub items_json: String,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_cycle_count_create(
 _path: CycleCountCreatePath,
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

 let content = cycle_count_create_page(&warehouses);
 let page_html = admin_page(
 is_htmx,
 "新建盘点",
 &claims,
 "inventory",
 CycleCountCreatePath::PATH,
 "库存管理",
 Some("新建盘点"),
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

#[require_permission("INVENTORY", "create")]
pub async fn create_cycle_count(
 _path: CycleCountCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<CreateCycleCountForm>,
) -> Result<axum::response::Response> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.cycle_count_service();

 let count_date = chrono::NaiveDate::parse_from_str(&form.count_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效日期格式: {e}")))?;

 let is_blind = form.is_blind.as_deref() == Some("on");
 let warehouse_id = form.warehouse_id
 .ok_or_else(|| DomainError::validation("请选择盘点仓库"))?;

 let web_items: Vec<CycleCountItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("物料数据无效: {e}")))?;

 let items: Vec<CreateCycleCountItemReq> = web_items.into_iter().map(|it| {
 let product_id: i64 = it.product_id.parse().unwrap_or(0);
 let bin_id: i64 = it.bin_id.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0);
 let system_qty: Decimal = it.system_qty.parse().unwrap_or(Decimal::ZERO);
 CreateCycleCountItemReq {
 bin_id,
 product_id,
 batch_no: it.batch_no.filter(|s| !s.is_empty()),
 system_qty,
 }
 }).collect();

 let req = CreateCycleCountReq {
 warehouse_id,
 zone_id: form.zone_id,
 count_date,
 is_blind,
 remark: form.remark,
 items,
 };

 let id = svc.create(&service_ctx, &mut conn, req).await?;

 if form.action.as_deref() == Some("start") {
 svc.start_count(&service_ctx, &mut conn, id).await?;
 }

 let redirect = CycleCountListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())).into_response())
}

// ── Components ──

fn cycle_count_create_page(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
 div {
 // ── Back Link ──
 a href=(format!("{}?restore=true", CycleCountListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回盘点列表"
 }
 // ── Page Header ──
 div class="flex items-center justify-between mb-5" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建盘点" }
 span class="text-xs text-muted flex items-center gap-2" {
 (icon::clock_icon("w-3.5 h-3.5"))
 "自动保存草稿"
 }
 }
 form hx-post=(CycleCountCreatePath::PATH) hx-swap="none" id="cycleCountForm"
 onsubmit="return cycleCountCollectItems()" {
 // ── 盘点信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::building_icon("w-[18px] h-[18px]"))
 "盘点信息"
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
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库区" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="zone_id" {
 option value="" { "全部库区" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "盘点日期 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="count_date" required {}
 }
 div class="flex flex-col gap-1" {
 span class="text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "盲盘模式" }
 label class="inline-flex items-center gap-2 cursor-pointer pt-2 text-sm text-fg-2" {
 input class="w-auto" type="checkbox" name="is_blind";
 "开启盲盘（隐藏系统数量）"
 }
 }
 div class="form-field col-span-2" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent resize-y" name="remark" rows="2" placeholder="可选备注…" {}
 }
 }
 }
 // ── 盘点物料 ──
 div class="form-section p-0 overflow-hidden" {
 div class="px-6 pt-6 pb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "盘点物料"
 span id="cc-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10 text-center" { "行号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格" }
 th class="w-[100px]" { "库位" }
 th class="w-[120px]" { "批次号" }
 th class="w-[100px] text-right" { "系统数量" }
 th class="w-10" { }
 }
 }
 tbody id="cc-item-tbody" { }
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
 input type="hidden" name="items_json" id="cc-items-json" value="[]" {}
 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 div { }
 div class="flex gap-3" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", CycleCountListPath::PATH)) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" name="action" value="draft" {
 "保存草稿"
 }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" name="action" value="start" {
 (icon::check_circle_icon("w-4 h-4"))
 "开始盘点"
 }
 }
 }
 }
 (crate::components::product_picker::product_picker_modal_with_search("product-modal", CycleCountItemRowPath::PATH, "cc-item-tbody"))
 // ── JS ──
 (maud::PreEscaped(r#"<script>
 function ccCalcSummary() {
 var tbody = document.getElementById('cc-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 document.getElementById('cc-item-count').textContent = '共 ' + rows.length + ' 项';
 }
 function ccRenumber() {
 var tbody = document.getElementById('cc-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 rows.forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 ccCalcSummary();
 }
 function cycleCountCollectItems() {
 var tbody = document.getElementById('cc-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 bin_id: row.querySelector('input[name="bin_id"]').value || null,
 batch_no: row.querySelector('input[name="batch_no"]').value || null,
 system_qty: row.querySelector('input[name="system_qty"]').value || '0'
 });
 });
 document.getElementById('cc-items-json').value = JSON.stringify(items);
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
 td {
 input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="number" name="bin_id" placeholder="库位ID" {}
 }
 td {
 input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="text" name="batch_no" placeholder="批次号" {}
 }
 td {
 input class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="number" min="0" step="any" name="system_qty" placeholder="0" {}
 }
 td {
 button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 _="on click remove closest <tr/> then call ccRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 }
 }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}
