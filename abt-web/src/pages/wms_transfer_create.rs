use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::transfer::TransferService;
use abt_core::wms::transfer::model::{CreateTransferReq, CreateTransferItemReq};
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_transfer::{TransferCreatePath, TransferListPath, TransferProductsPath, TransferItemRowPath};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
 pub name: Option<String>,
 pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_transfer_create(
 _path: TransferCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = transfer_create_page(&warehouses);
 let page_html = admin_page(
 is_htmx, "新建调拨单", &claims, "inventory", TransferCreatePath::PATH, "库存管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

/// HTMX: search products
#[require_permission("PRODUCT", "read")]
pub async fn get_products(
 ctx: RequestContext,
 Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.product_service();

 let filter = ProductQuery {
 name: params.name.filter(|s| !s.is_empty()),
 code: params.code.filter(|s| !s.is_empty()),
 status: None,
 owner_department_id: None,
 category_id: None,
 };
 let result = svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 20)).await?;
 Ok(Html(product_list_fragment(&result.items).into_string()))
}

/// HTMX: return a single item row
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
struct TransferItemWeb {
 product_id: String,
 quantity: String,
 batch_no: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct TransferCreateForm {
 #[serde(deserialize_with = "empty_as_none")]
 pub from_warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub from_zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub from_bin_id: Option<i64>,
 #[serde(deserialize_with = "empty_as_none")]
 pub to_warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub to_zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub to_bin_id: Option<i64>,
 pub transfer_date: NaiveDate,
 pub remark: Option<String>,
 pub items_json: String,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_transfer(
 _path: TransferCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<TransferCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.transfer_service();

 let from_warehouse_id = form.from_warehouse_id
 .ok_or_else(|| DomainError::validation("请选择调出仓库"))?;
 let to_warehouse_id = form.to_warehouse_id
 .ok_or_else(|| DomainError::validation("请选择调入仓库"))?;

 let web_items: Vec<TransferItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;

 if web_items.is_empty() {
 return Err(DomainError::validation("调拨单至少需要一条明细").into());
 }

 let items: Vec<CreateTransferItemReq> = web_items.into_iter().map(|item| {
 CreateTransferItemReq {
 product_id: item.product_id.parse().unwrap_or(0),
 quantity: item.quantity.parse().unwrap_or(Decimal::ZERO),
 batch_no: item.batch_no,
 }
 }).collect();

 let req = CreateTransferReq {
 from_warehouse_id,
 from_zone_id: form.from_zone_id,
 from_bin_id: form.from_bin_id,
 to_warehouse_id,
 to_zone_id: form.to_zone_id,
 to_bin_id: form.to_bin_id,
 transfer_date: form.transfer_date,
 items,
 };

 let _id = svc.create(&service_ctx, &mut conn, req).await?;

 let redirect = TransferListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn transfer_create_page(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
 div {
 a href="/admin/wms/transfers" class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回库存调拨列表"
 }

 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建调拨单" }
 }

 // ── Workflow Preview ──
 div class="flex items-center" {
 div class="flex items-center gap-2 text-xs text-muted current" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "草稿" }
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 text-xs text-muted" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "在途" }
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 text-xs text-muted" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "已完成" }
 }

 form hx-post=(TransferCreatePath::PATH) hx-swap="none"
 onsubmit="return transferCollectItems()" {
 // ── From / To Warehouse ──
 div class="bg-bg border border-border rounded p-6" {
 h3 class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::building_icon("w-[18px] h-[18px]"))
 "调拨信息"
 }
 div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调出仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="from_warehouse_id" required {
 option value="" { "请选择仓库" }
 @for wh in warehouses {
 option value=(wh.id) { (wh.name) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调出库区" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="from_zone_id" {
 option value="" { "按策略分配" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调出储位" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="from_bin_id" {
 option value="" { "按策略分配" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调入仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="to_warehouse_id" required {
 option value="" { "请选择仓库" }
 @for wh in warehouses {
 option value=(wh.id) { (wh.name) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调入库区" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="to_zone_id" {
 option value="" { "按策略分配" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调入储位" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="to_bin_id" {
 option value="" { "按策略分配" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调拨日期 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="transfer_date" value="2026-06-06" required {}
 }
 }
 }

 // ── Line Items ──
 div class="bg-bg border border-border rounded p-6" {
 h3 class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "调拨明细"
 span id="transfer-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
 }
 table class="data-table" {
 thead {
 tr {
 th style="width:40px" { "序号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格型号" }
 th style="width:100px" { "调拨数量 " span class="required" { "*" } }
 th { "批次号" }
 th style="width:40px" { }
 }
 }
 tbody id="transfer-item-tbody" { }
 }
 div style="margin-top:var(--space-4)" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-[#2563eb] text-sm font-medium cursor-pointer"
 _="on click add .is-open to #transfer-product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加物料"
 }
 }
 }

 // ── Remark ──
 div class="bg-bg border border-border rounded p-6" {
 h3 class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::edit_icon("w-[18px] h-[18px]"))
 "备注"
 }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" placeholder="输入备注信息…" rows="3" style="width:100%;min-height:80px;padding:var(--space-2) var(--space-3);resize:vertical" { }
 }

 input type="hidden" name="items_json" id="transfer-items-json" value="[]" {}

 // ── Actions ──
 div class="flex items-center justify-end gap-3 pt-4 [border-top:1px_solid_var(--border-soft)]" {
 a href="/admin/wms/transfers" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "提交调拨"
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("transfer-product-modal", TransferProductsPath::PATH))

 // ── Line Item JS ──
 (maud::PreEscaped(r#"<script>
 function transferCollectItems() {
 var tbody = document.getElementById('transfer-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 quantity: row.querySelector('input[name="quantity"]').value || '0',
 batch_no: row.querySelector('input[name="batch_no"]').value || null
 });
 });
 document.getElementById('transfer-items-json').value = JSON.stringify(items);
 if (items.length === 0) { alert('请至少添加一个物料'); return false; }
 return true;
 }
 function transferRenumber() {
 var tbody = document.getElementById('transfer-item-tbody');
 tbody.querySelectorAll('tr').forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 }
 </script>"#))
 }
}

/// Product search results fragment
fn product_list_fragment(products: &[abt_core::master_data::product::model::Product]) -> Markup {
 html! {
 @if products.is_empty() {
 div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
 (icon::package_icon("w-8 h-8"))
 p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的产品" }
 }
 } @else {
 div class="py-2" {
 @for p in products {
 div class="flex items-center p-3 [border-bottom:1px_solid_var(--border-soft)] cursor-pointer hover:bg-accent-bg transition-colors"
                    hx-get=(format!("{}?product_id={}", TransferItemRowPath::PATH, p.product_id))
                    hx-target="#transfer-item-tbody"
                    hx-swap="beforeend"
                    _="on 'htmx:afterRequest' remove .is-open from #transfer-product-modal" {
 div class="flex-1 min-w-0" {
 div class="text-sm font-medium text-fg" { (p.pdt_name) }
 div class="text-[12px] text-muted flex items-center gap-[6px] flex-wrap" {
 span class="bg-surface rounded-sm" { (p.product_code) }
 span class="text-border" { "·" }
 span { (p.meta.specification) }
 span class="text-border" { "·" }
 span { (p.unit) }
 }
 }
 span class="text-xs text-accent font-medium shrink-0" { "点击添加" }
 }
 }
 }
 }
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
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0.01" step="any" name="quantity" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" placeholder="批次号" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/> then call transferRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}
