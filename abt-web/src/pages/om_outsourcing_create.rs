use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::om::enums::OutsourcingType;
use abt_core::om::outsourcing_order::{CreateOutsourcingOrderReq, OutsourcingMaterialItem, OutsourcingOrderService};
use abt_core::shared::types::PageParams;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_order::model::WorkOrderFilter;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{OmOutsourcingCreatePath, OmOutsourcingDetailPath, OmOutsourcingListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form structs ──

#[derive(Debug, Deserialize)]
pub struct CreateForm {
 pub supplier_id: i64,
 pub product_id: i64,
 pub outsourcing_type: i16,
 pub work_order_id: Option<i64>,
 pub routing_id: Option<i64>,
 pub planned_qty: String,
 pub unit_price: String,
 pub scheduled_date: Option<String>,
 pub virtual_warehouse_id: i64,
 pub remark: Option<String>,
 pub materials_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MaterialItemWeb {
 product_id: i64,
 planned_qty: String,
 unit_cost: Option<String>,
}

// ── Handlers ──

#[require_permission("OUTSOURCING", "create")]
pub async fn get_create(
 _path: OmOutsourcingCreatePath,
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
 let product_svc = state.product_service();
 let warehouse_svc = state.warehouse_service();
 let wo_svc = state.work_order_service();

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

 let products = product_svc
 .list(
 &service_ctx,
 &mut conn,
 ProductQuery {
 name: None,
 code: None,
 status: None,
 owner_department_id: None,
 category_id: None,
 },
 PageParams::new(1, 200),
 )
 .await?;

 let warehouses = warehouse_svc
 .list(
 &service_ctx,
 &mut conn,
 WarehouseFilter {
 warehouse_type: None,
 status: None,
 keyword: None,
 },
 1,
 200,
 )
 .await?;

 let work_orders = wo_svc
 .list(
 &service_ctx,
 &mut conn,
 WorkOrderFilter {
 status: None,
 product_id: None,
 keyword: None,
 date_from: None,
 date_to: None,
 },
 1,
 200,
 )
 .await?;

 let content = create_page(
 &suppliers.items,
 &products.items,
 &warehouses.items,
 &work_orders.items,
 );

 let page_html = admin_page(
 is_htmx,
 "新建委外单",
 &claims,
 "outsourcing",
 OmOutsourcingCreatePath::PATH,
 "委外管理",
 Some(OmOutsourcingListPath::PATH),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("OUTSOURCING", "create")]
pub async fn create(
 _path: OmOutsourcingCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<CreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.outsourcing_order_service();

 let outsourcing_type = OutsourcingType::from_i16(form.outsourcing_type)
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效委外类型"))?;

 let scheduled_date = form
 .scheduled_date
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|s| {
 chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
 .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效日期格式: {e}")))
 })
 .transpose()?;

 let materials: Vec<OutsourcingMaterialItem> = form
 .materials_json
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|json| {
 let web_items: Vec<MaterialItemWeb> = serde_json::from_str(json)
 .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效物料数据: {e}")))?;
 Ok::<Vec<OutsourcingMaterialItem>, abt_core::shared::types::DomainError>(web_items
 .into_iter()
 .map(|item| OutsourcingMaterialItem {
 product_id: item.product_id,
 planned_qty: item
 .planned_qty
 .parse()
 .unwrap_or(rust_decimal::Decimal::ZERO),
 unit_cost: item
 .unit_cost
 .and_then(|s| s.parse().ok()),
 })
 .collect())
 })
 .transpose()?
 .unwrap_or_default();

 let req = CreateOutsourcingOrderReq {
 work_order_id: form.work_order_id,
 routing_id: form.routing_id,
 supplier_id: form.supplier_id,
 product_id: form.product_id,
 outsourcing_type,
 planned_qty: form
 .planned_qty
 .parse()
 .map_err(|_| abt_core::shared::types::DomainError::validation("无效计划数量"))?,
 unit_price: form
 .unit_price
 .parse()
 .map_err(|_| abt_core::shared::types::DomainError::validation("无效单价"))?,
 scheduled_date,
 virtual_warehouse_id: form.virtual_warehouse_id,
 remark: form.remark,
 materials,
 };

 let id = svc.create(&service_ctx, &mut conn, req, None).await?;

 let redirect = OmOutsourcingDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Components ──

fn create_page(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 products: &[abt_core::master_data::product::model::Product],
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 work_orders: &[abt_core::mes::work_order::model::WorkOrder],
) -> Markup {
 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" href=(format!("{}?restore=true", OmOutsourcingListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回委外单列表"
 }
 // ── Page Header ──
 div class="flex items-center justify-between mb-5" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建委外单" }
 }

 form
 id="om-create-form"
 hx-post=(OmOutsourcingCreatePath::PATH)
 hx-swap="none"
 {
 // ── Section 1: 基本信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::clipboard_document_icon("w-[18px] h-[18px]"))
 "基本信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "委外单号" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted outline-none" type="text" value="自动生成" readonly;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="supplier_id" required {
 option value="" { "请选择供应商" }
 @for s in suppliers {
 option value=(s.id) { (s.name) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="product_id" required {
 option value="" { "请选择产品" }
 @for p in products {
 option value=(p.product_id) {
 (p.pdt_name) " (" (p.product_code) ")"
 }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "委外类型 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="outsourcing_type" required {
 option value="" { "请选择委外类型" }
 option value="1" { "整体委外" }
 option value="2" { "工序委外" }
 option value="3" { "材料委外" }
 option value="4" { "返工委外" }
 }
 }
 }
 }

 // ── Section 2: 关联信息与数量 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::link_icon("w-[18px] h-[18px]"))
 "关联信息与数量"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工单" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="work_order_id" {
 option value="" { "请选择工单" }
 @for wo in work_orders {
 option value=(wo.id) { (wo.doc_number) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工序" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" name="routing_id" placeholder="请输入工序ID";
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "计划数量 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="0.01" min="0" name="planned_qty" required;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "单价 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" min="0" name="unit_price" required;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "预计交期" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="scheduled_date";
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "虚拟仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="virtual_warehouse_id" required {
 option value="" { "请选择仓库" }
 @for w in warehouses {
 @if w.is_virtual {
 option value=(w.id) { (w.name) }
 }
 }
 }
 }
 }
 }

 // ── Section 3: 发料明细 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "发料明细"
 }
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead { tr {
 th { "物料" }
 th { "应发数量" }
 th { "单位成本" }
 th { "小计" }
 th style="width:50px" { }
 }}
 tbody id="material-tbody" { }
 }
 }
 input type="hidden" name="materials_json" id="materials-json" value="";
 }
 div class="p-4 flex items-center gap-2" {
 button type="button" class="inline-flex items-center gap-2 text-accent text-sm font-medium cursor-pointer hover:text-accent-hover transition-colors duration-150"
 _="on click call omAddMaterialRow()" {
 (icon::plus_icon("w-4 h-4"))
 "添加物料"
 }
 }
 }

 // ── Section 4: 备注 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::comment_icon("w-[18px] h-[18px]"))
 "备注"
 }
 div class="form-field col-span-2" {
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent resize-y" name="remark" rows="3" placeholder="请输入备注信息…" {}
 }
 }

 // ── Action bar ──
 div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 div { }
 div class="flex gap-3" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", OmOutsourcingListPath::PATH)) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "确认提交"
 }
 }
 }
 }

 // ── Material row modal ──
 div id="material-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 is-open:opacity-100 is-open:pointer-events-auto" _="on click[me is event.target] remove .is-open from #material-modal" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" _="on click halt the event" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h3 class="text-base font-semibold text-fg" { "选择物料" }
 button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-surface transition-colors duration-150" title="关闭"
 _="on click remove .is-open from #material-modal" {
 (icon::x_icon("w-4 h-4"))
 }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field col-span-2" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "物料" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" id="modal-product-id" {
 option value="" { "请选择物料" }
 @for p in products {
 option value=(p.product_id) {
 (p.pdt_name) " (" (p.product_code) ")"
 }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "应发数量 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="0.01" min="0" id="modal-planned-qty" required;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "单位成本" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" min="0" id="modal-unit-cost";
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .is-open from #material-modal" { "取消" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 _="on click call omConfirmMaterial()" { "确认" }
 }
 }
 }

 // ── Inline scripts ──
 script {
 (maud::PreEscaped(r#"
function omAddMaterialRow() {
 document.querySelector('#modal-product-id').value = '';
 document.querySelector('#modal-planned-qty').value = '';
 document.querySelector('#modal-unit-cost').value = '';
 document.querySelector('#material-modal').classList.toggle('is-open');
}

function omConfirmMaterial() {
 var sel = document.querySelector('#modal-product-id');
 var pid = sel.value;
 var pname = sel.options[sel.selectedIndex] ? sel.options[sel.selectedIndex].textContent.trim() : '';
 var qty = parseFloat(document.querySelector('#modal-planned-qty').value) || 0;
 var cost = parseFloat(document.querySelector('#modal-unit-cost').value) || 0;
 if (!pid || qty <= 0) return;

 var tbody = document.querySelector('#material-tbody');
 var tr = document.createElement('tr');
 tr.setAttribute('oninput','omUpdateMaterialJson()');
 tr.innerHTML = '<td>' + pname + '<input type="hidden" name="m_product_id" value="' + pid + '"></td>' +
 '<td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" min="0" name="m_planned_qty" value="' + qty + '" class="w-[100px] text-right"></td>' +
 '<td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" min="0" name="m_unit_cost" value="' + cost + '" class="w-[100px] text-right"></td>' +
 '<td class="line-subtotal font-mono tabular-nums text-right">' + (qty * cost).toFixed(2) + '</td>' +
 '<td><button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-surface transition-colors duration-150" title="删除" onclick="this.closest(\'tr\').remove();omUpdateMaterialJson()">' + '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button></td>';
 tbody.appendChild(tr);
 omUpdateMaterialJson();
 document.querySelector('#material-modal').classList.remove('is-open');
}

function omUpdateMaterialJson() {
 var rows = Array.from(document.querySelectorAll('#material-tbody tr'));
 var items = [];
 rows.forEach(function(tr) {
 var pid = tr.querySelector('[name=m_product_id]');
 var qty = tr.querySelector('[name=m_planned_qty]');
 var cost = tr.querySelector('[name=m_unit_cost]');
 if (pid && qty) {
 var q = parseFloat(qty.value) || 0;
 var c = cost ? (parseFloat(cost.value) || 0) : 0;
 tr.querySelector('.line-subtotal').textContent = (q * c).toFixed(2);
 items.push({
 product_id: parseInt(pid.value),
 planned_qty: qty.value,
 unit_cost: cost && cost.value ? cost.value : null
 });
 }
 });
 document.querySelector('#materials-json').value = JSON.stringify(items);
}
 "#))
 }
 }
}
 }
