use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_bin::{BinCreatePath, BinDetailPath, BinListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Form Data ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct BinCreateForm {
 pub code: String,
 pub name: String,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub zone_id: Option<i64>,
 pub row_no: Option<String>,
 pub column_no: Option<String>,
 pub layer_no: Option<String>,
 pub capacity_limit: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::multi_string")]
 pub allowed_product_types: Vec<String>,
 pub temperature_req: Option<String>,
}

// ── Handlers ──

#[require_permission("LOCATION", "read")]
pub async fn get_bin_create(
 _path: BinCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.warehouse_service();

 let warehouses = svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await?;

 // Load zones for all warehouses (for dependent dropdown)
 let mut all_zones: Vec<(i64, Vec<Zone>)> = Vec::new();
 for wh in &warehouses.items {
 if let Ok(zs) = svc.list_zones(&service_ctx, &mut conn, wh.id).await {
 all_zones.push((wh.id, zs));
 }
 }

 let content = bin_create_page(&warehouses.items, &all_zones);
 let page_html = admin_page(
 is_htmx,
 "新建库位",
 &claims,
 "inventory",
 BinCreatePath::PATH,
 "库存管理",
 Some("新建库位"),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

#[require_permission("LOCATION", "create")]
pub async fn create_bin(
 _path: BinCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<BinCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.warehouse_service();

 let zone_id = form.zone_id
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("请选择所属库区"))?;

 let capacity_limit = form.capacity_limit
 .filter(|s| !s.is_empty())
 .and_then(|s| s.parse::<Decimal>().ok());

 let create_req = CreateBinReq {
 code: form.code,
 name: form.name,
 row_no: form.row_no.filter(|s| !s.is_empty()),
 column_no: form.column_no.filter(|s| !s.is_empty()),
 layer_no: form.layer_no.filter(|s| !s.is_empty()),
 capacity_limit,
 allowed_product_types: if form.allowed_product_types.is_empty() { None } else { Some(form.allowed_product_types) },
 temperature_req: form.temperature_req.filter(|s| !s.is_empty()),
 };

 let bin_id = svc.create_bin(&service_ctx, &mut conn, zone_id, create_req).await?;

 let redirect = BinDetailPath { id: bin_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn bin_create_page(
 warehouses: &[Warehouse],
 all_zones: &[(i64, Vec<Zone>)],
) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", BinListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回库位管理列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建库位" }
 }

 form id="bin-create-form"
 hx-post=(BinCreatePath::PATH)
 hx-swap="none" {

 // ── Section: 库位信息 ──
 div class="data-card mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::grid_icon("w-4 h-4"))
 " 库位信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "所属仓库 " span class="text-danger" { "*" } }
 select name="warehouse_id" required id="warehouse-select"
 onchange="updateZones()" {
 option value="" disabled selected { "-- 请选择 --" }
 @for wh in warehouses {
 option value=(wh.id) { (wh.name) }
 }
 }
 }
 div class="form-field" {
 label { "所属库区 " span class="text-danger" { "*" } }
 select name="zone_id" required id="zone-select" {
 option value="" { "请先选择仓库" }
 @for (wh_id, zones) in all_zones {
 @for z in zones {
 option value=(z.id) data-wh=(wh_id) style="display:none" { (z.code) " " (z.name) }
 }
 }
 }
 }
 div class="form-field" {
 label { "库位编码 " span class="text-danger" { "*" } }
 input type="text" name="code" required placeholder="如 A01-R01-C01-L01";
 }
 div class="form-field" {
 label { "库位名称 " span class="text-danger" { "*" } }
 input type="text" name="name" required placeholder="如 A区1排1列";
 }
 div class="form-field" {
 label { "行号" }
 input type="number" name="row_no" placeholder="1";
 }
 div class="form-field" {
 label { "列号" }
 input type="number" name="column_no" placeholder="1";
 }
 div class="form-field" {
 label { "层号" }
 input type="number" name="layer_no" placeholder="1";
 }
 div class="form-field" {
 label { "容量上限" }
 input type="number" name="capacity_limit" placeholder="请输入容量上限";
 }
 div class="form-field" {
 label { "温控要求" }
 select name="temperature_req" {
 option value="" { "无要求" }
 option value="ambient" { "常温" }
 option value="cool" { "冷藏 (2~8°C)" }
 option value="freeze" { "冷冻 (-18°C以下)" }
 option value="constant" { "恒温" }
 }
 }
 div class="col-span-2 flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "允许物料类型" }
 div class="flex flex-wrap gap-2.5 pt-1" {
 label class="flex items-center gap-1 text-sm text-fg-2 cursor-pointer" {
 input type="checkbox" name="allowed_product_types" value="raw_material" class="accent-accent w-4 h-4 cursor-pointer" checked;
 "原材料"
 }
 label class="flex items-center gap-1 text-sm text-fg-2 cursor-pointer" {
 input type="checkbox" name="allowed_product_types" value="semi_finished" class="accent-accent w-4 h-4 cursor-pointer";
 "半成品"
 }
 label class="flex items-center gap-1 text-sm text-fg-2 cursor-pointer" {
 input type="checkbox" name="allowed_product_types" value="finished" class="accent-accent w-4 h-4 cursor-pointer";
 "成品"
 }
 label class="flex items-center gap-1 text-sm text-fg-2 cursor-pointer" {
 input type="checkbox" name="allowed_product_types" value="packaging" class="accent-accent w-4 h-4 cursor-pointer";
 "包材"
 }
 label class="flex items-center gap-1 text-sm text-fg-2 cursor-pointer" {
 input type="checkbox" name="allowed_product_types" value="consumable" class="accent-accent w-4 h-4 cursor-pointer";
 "耗材"
 }
 }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", BinListPath::PATH)) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "保存库位"
 }
 }
 }

 // ── Warehouse-Zone dependent dropdown script ──
 script {
 (maud::PreEscaped(r#"
 function updateZones() {
 var whId = document.getElementById('warehouse-select').value;
 var zoneSelect = document.getElementById('zone-select');
 var options = zoneSelect.querySelectorAll('option[data-wh]');
 options.forEach(function(opt) {
 opt.style.display = (!whId || opt.dataset.wh === whId) ? '' : 'none';
 });
 zoneSelect.value = '';
 }
 "#))
 }
 }
 }
}
