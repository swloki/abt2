use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::wms::warehouse::model::{CreateWarehouseReq, Warehouse};
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::enums::WarehouseType;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_warehouse::{
 WarehouseCreatePath, WarehouseDetailPath, WarehouseEditPath, WarehouseListPath,
};
use crate::utils::{RequestContext, empty_as_none};

use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct WarehouseCreateForm {
 pub code: String,
 pub name: String,
 pub warehouse_type: i16,
 pub is_virtual: Option<String>,
 pub address: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub manager_id: Option<i64>,
 pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("WAREHOUSE", "read")]
pub async fn get_warehouse_create(
 _path: WarehouseCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, .. } = ctx;

 let content = warehouse_create_page(None);
 let page_html = admin_page(
 is_htmx,
 "新建仓库",
 &claims,
 "inventory",
 WarehouseCreatePath::PATH,
 "库存管理",
 Some("新建仓库"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("WAREHOUSE", "create")]
pub async fn create_warehouse(
 _path: WarehouseCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<WarehouseCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.warehouse_service();

 let warehouse_type = WarehouseType::from_i16(form.warehouse_type)
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效的仓库类型"))?;

 let is_virtual = form.is_virtual.is_some();

 if form.code.trim().is_empty() {
 return Err(abt_core::shared::types::DomainError::validation("仓库编码不能为空").into());
 }
 if form.name.trim().is_empty() {
 return Err(abt_core::shared::types::DomainError::validation("仓库名称不能为空").into());
 }
 let create_req = CreateWarehouseReq {
 code: form.code,
 name: form.name,
 warehouse_type,
 address: if is_virtual { None } else { form.address.filter(|s| !s.is_empty()) },
 manager_id: form.manager_id,
 is_virtual,
 remark: form.remark.filter(|s| !s.is_empty()).unwrap_or_default(),
 };
 let warehouse_id = svc.create(&service_ctx, &mut conn, create_req).await?;

 let redirect = WarehouseDetailPath { id: warehouse_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}



// ── Components ──

pub(crate) fn warehouse_create_page(warehouse: Option<&Warehouse>) -> Markup {
 let is_edit = warehouse.is_some();
 let title = if is_edit { "编辑仓库" } else { "新建仓库" };
 let form_action = if let Some(w) = warehouse {
 WarehouseEditPath { id: w.id }.to_string()
 } else {
 WarehouseCreatePath::PATH.to_string()
 };

 let (code_val, name_val, type_val, is_virtual, address_val, remark_val) = match warehouse {
 Some(w) => (
 w.code.clone(),
 w.name.clone(),
 match w.warehouse_type {
 WarehouseType::RawMaterial => 1,
 WarehouseType::FinishedGoods => 2,
 WarehouseType::SemiFinished => 3,
 WarehouseType::Consumable => 4,
 WarehouseType::VirtualOutsource => 5,
 },
 w.is_virtual,
 w.address.clone().unwrap_or_default(),
 w.remark.clone(),
 ),
 None => (String::new(), String::new(), 0, false, String::new(), String::new()),
 };

 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", WarehouseListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回仓库管理列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { (title) }
 }

 form id="warehouse-form"
 hx-post=(form_action)
 hx-swap="none"
 class="space-y-5" {

 // ── Section: 基本信息 ──
 div class="data-card mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::building_icon("w-4 h-4"))
 " 基本信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "仓库编码 " span class="text-danger" { "*" } }
 input type="text" name="code" required placeholder="如 WH-007"
 value=(code_val);
 }
 div class="form-field" {
 label { "仓库名称 " span class="text-danger" { "*" } }
 input type="text" name="name" required placeholder="请输入仓库名称"
 value=(name_val);
 }
 div class="form-field" {
 label { "仓库类型 " span class="text-danger" { "*" } }
 select name="warehouse_type" required
 id="warehouse-type-select" {
 option value="" disabled selected[type_val == 0] { "-- 请选择 --" }
 option value="1" selected[type_val == 1] { "原材料仓" }
 option value="2" selected[type_val == 2] { "成品仓" }
 option value="3" selected[type_val == 3] { "半成品仓" }
 option value="4" selected[type_val == 4] { "辅料仓" }
 option value="5" selected[type_val == 5] { "虚拟委外仓" }
 }
 }
 div class="form-field" {
 label { "管理员" }
 input type="text" name="manager_display" placeholder="请选择管理员"
 class="bg-surface text-muted" readonly;
 input type="hidden" name="manager_id";
 }
 div class="form-field" id="address-field" {
 label { "地址" }
 input type="text" name="address" placeholder="请输入仓库地址"
 value=(address_val);
 }
 div class="form-field flex items-end pb-1" {
 label class="flex items-center cursor-pointer m-0 gap-2" {
 input type="checkbox" name="is_virtual" value="true"
 id="is-virtual-checkbox"
 checked[is_virtual];
 "是否虚拟仓库（委外）"
 }
 }
 div id="virtual-tip" class="col-span-full"
 style=(if is_virtual { "display:block" } else { "display:none" }) {
 div class="text-fg-2 rounded-md px-5 py-4 text-sm bg-accent-bg border border-[rgba(22,119,255,0.15)] leading-relaxed" {
 div class="flex items-center font-semibold text-accent gap-2 mb-1" {
 (icon::circle_alert_icon("w-4 h-4"))
 "虚拟委外仓说明"
 }
 "虚拟仓库不对应实际物理空间，用于管理委外加工物料。类型将自动设为「虚拟委外仓」，地址字段无需填写。委外发料/收货通过库存调拨实现。"
 }
 }
 div class="form-field field-full" {
 label { "备注" }
 textarea name="remark" placeholder="输入仓库相关备注信息…"
 class="w-full resize-y min-h-[80px]" { (remark_val) }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", WarehouseListPath::PATH)) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "保存仓库"
 }
 }
 }

 // ── Virtual warehouse toggle script ──
 script {
 r#"
 (function() {
 var cb = document.getElementById('is-virtual-checkbox');
 var tip = document.getElementById('virtual-tip');
 var addrField = document.getElementById('address-field');
 var typeSelect = document.getElementById('warehouse-type-select');
 function toggle() {
 var checked = cb.checked;
 tip.style.display = checked ? 'block' : 'none';
 addrField.style.display = checked ? 'none' : '';
 if (checked) typeSelect.value = '5';
 }
 cb.addEventListener('change', toggle);
 })();
 "#
 }
 }
 }
}
