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
use crate::routes::om::{
    OmOutsourcingCreatePath, OmOutsourcingDetailPath, OmOutsourcingListPath,
    OmOutsourcingSuggestMaterialsPath, OmOutsourcingWoSummaryPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form structs ──

#[derive(Debug, Deserialize)]
pub struct CreateForm {
 pub supplier_id: i64,
 pub product_id: i64,
 pub outsourcing_type: i16,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub work_order_id: Option<i64>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub routing_id: Option<i64>,
 pub planned_qty: String,
 pub unit_price: String,
 pub scheduled_date: Option<String>,
 pub virtual_warehouse_id: i64,
 pub source_warehouse_id: i64,
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

 // min_pack_qty 二次校验（防前端绕过）
 if !materials.is_empty() {
 use abt_core::master_data::product::ProductService;
 let pids: Vec<i64> = materials.iter().map(|m| m.product_id).collect();
 let prods = state.product_service().get_by_ids(&service_ctx, &mut conn, pids).await?;
 for m in &materials {
 if let Some(p) = prods.iter().find(|p| p.product_id == m.product_id) {
 if let Some(mp) = p.min_pack_qty {
 if mp > rust_decimal::Decimal::ZERO && (m.planned_qty % mp) != rust_decimal::Decimal::ZERO {
 return Err(abt_core::shared::types::DomainError::validation(format!(
 "物料 {} 需求数量 {} 必须是最小包装数量 {} 的整数倍",
 p.product_code, m.planned_qty, mp
 )).into());
 }
 }
 }
 }
 }

 let req = CreateOutsourcingOrderReq {
 work_order_id: form.work_order_id,
 routing_id: form.routing_id,
 process_name: None, // B2 从选定工序填入
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
 source_warehouse_id: form.source_warehouse_id,
 remark: form.remark,
 materials,
 };

 let id = svc.create(&service_ctx, &mut conn, req, None).await?;

 let redirect = OmOutsourcingDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── 联动：关联工单摘要（回填产品/数量/交期 + 渲染工序下拉）──

#[derive(Debug, Deserialize)]
pub struct WoSummaryQuery {
 pub work_order_id: i64,
}

#[require_permission("OUTSOURCING", "read")]
pub async fn wo_summary(
 _path: OmOutsourcingWoSummaryPath,
 ctx: RequestContext,
 axum::extract::Query(q): axum::extract::Query<WoSummaryQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.outsourcing_order_service();
 let s = svc.outsourcing_summary(&service_ctx, &mut conn, q.work_order_id).await?;
 Ok(Html(routing_select_fragment(&s).into_string()))
}

/// 渲染关联工序下拉 + 回填脚本。默认只列 is_outsourced=true，checkbox 切换全部。
fn routing_select_fragment(s: &abt_core::om::outsourcing_order::model::WorkOrderOutsourcingSummary) -> Markup {
 // 默认只列可委外工序
 let outsourced: Vec<_> = s.routings.iter().filter(|r| r.is_outsourced).collect();
 let all_json = serde_json::to_string(
 &s.routings.iter().map(|r| (r.id, r.step_no, r.process_name.clone(), r.is_outsourced)).collect::<Vec<_>>(),
 ).unwrap_or_else(|_| "[]".into());
 html! {
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
     name="routing_id" id="routing-select"
     hx-get=(OmOutsourcingSuggestMaterialsPath::PATH)
     hx-trigger="change"
     hx-target="#material-tbody"
     hx-swap="innerHTML"
     hx-include="[name='planned_qty'], [name='source_warehouse_id'], [name='work_order_id']" {
 @if outsourced.is_empty() {
 option value="" { "（无可委外工序，勾选下方显示全部）" }
 } @else {
 option value="" { "请选择工序" }
 }
 @for r in &outsourced {
 option value=(r.id) data-name=(r.process_name) { (r.step_no) " - " (r.process_name) }
 }
 }
 label class="inline-flex items-center gap-1 text-xs text-muted mt-1 cursor-pointer" {
 input type="checkbox" id="show-all-routings" class="cursor-pointer accent-accent";
 "显示全部工序"
 }
 input type="hidden" id="routings-json" value=(all_json);
 (maud::PreEscaped(format!(r#"
 <script>
 (function() {{
 var setVal = function(sel, v) {{ var el = document.querySelector(sel); if (el) el.value = v; }};
 setVal('[name="product_id"]', '{pid}');
 setVal('[name="planned_qty"]', '{pq}');
 setVal('[name="scheduled_date"]', '{se}');
 var cb = document.getElementById('show-all-routings');
 var data = JSON.parse(document.getElementById('routings-json').value || '[]');
 function rebuild(all) {{
 var sel = document.getElementById('routing-select');
 var cur = sel.value;
 sel.innerHTML = '<option value="">请选择工序</option>';
 data.forEach(function(r) {{
 if (all || r[3]) {{
 var o = document.createElement('option');
 o.value = r[0]; o.dataset.name = r[2];
 o.textContent = r[1] + ' - ' + r[2];
 sel.appendChild(o);
 }});
 sel.value = cur;
 }});
 if (cb) cb.addEventListener('change', function() {{ rebuild(cb.checked); }});
 rebuild(false);
 }})();
 </script>
 "#, pid = s.product_id, pq = s.planned_qty, se = s.scheduled_end)))
 }
}

// ── 联动：发料即时查询（BOM 展开 + 库存 + min_pack_qty）──

#[derive(Debug, Deserialize)]
pub struct SuggestQuery {
 pub work_order_id: i64,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub routing_id: Option<i64>,
 pub planned_qty: rust_decimal::Decimal,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub source_warehouse_id: Option<i64>,
}

#[require_permission("OUTSOURCING", "read")]
pub async fn suggest_materials(
 _path: OmOutsourcingSuggestMaterialsPath,
 ctx: RequestContext,
 axum::extract::Query(q): axum::extract::Query<SuggestQuery>,
) -> Result<Html<String>> {
 use abt_core::master_data::bom::BomQueryService;
 use abt_core::master_data::product::ProductService;
 use abt_core::mes::production_batch::ProductionBatchService;
 use abt_core::wms::stock_ledger::StockLedgerService;
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let routing_id = q.routing_id
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("请先选择关联工序"))?;
 let batch_svc = state.production_batch_service();
 let routings = batch_svc.list_routings(&service_ctx, &mut conn, q.work_order_id).await?;
 let routing = routings.iter().find(|r| r.id == routing_id)
 .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
 let semi_product_id = routing.product_id
 .ok_or_else(|| abt_core::shared::types::DomainError::business_rule(
 "该工序未关联产出品，请先在工单工序维护设置产出品"
 ))?;

 // 半成品 → product_code → BOM 展开
 let product_svc = state.product_service();
 let semi = product_svc.get_by_ids(&service_ctx, &mut conn, vec![semi_product_id]).await?
 .into_iter().next()
 .ok_or_else(|| abt_core::shared::types::DomainError::not_found("Product"))?;
 let semi_code = semi.product_code.clone();

 let bom_svc = state.bom_query_service();
 let reqs = bom_svc.explode_for_procurement(&service_ctx, &mut conn, &semi_code, q.planned_qty).await?;
 if reqs.is_empty() {
 return Ok(Html(html! { tr { td colspan="5" class="text-center text-muted text-sm py-4" {
 "产出品无已发布 BOM 或无物料子件" } } }.into_string()));
 }

 // 批量取物料详情 + 库存
 let pids: Vec<i64> = reqs.iter().map(|r| r.product_id).collect();
 let products = product_svc.get_by_ids(&service_ctx, &mut conn, pids).await?;
 let stock_svc = state.stock_ledger_service();

 let mut rows: Vec<(i64, String, String, rust_decimal::Decimal, Option<rust_decimal::Decimal>, rust_decimal::Decimal)> = Vec::new();
 for r in &reqs {
 let p = products.iter().find(|p| p.product_id == r.product_id);
 let code = p.map(|p| p.product_code.clone()).unwrap_or_default();
 let name = p.map(|p| p.pdt_name.clone()).unwrap_or_default();
 let min_pack = p.and_then(|p| p.min_pack_qty);
 let stock = stock_svc.query_available(&service_ctx, &mut conn, r.product_id, q.source_warehouse_id).await.unwrap_or_default();
 rows.push((r.product_id, code, name, r.required_qty, min_pack, stock));
 }

 Ok(Html(material_rows_fragment(&rows).into_string()))
}

fn material_rows_fragment(
 rows: &[(i64, String, String, rust_decimal::Decimal, Option<rust_decimal::Decimal>, rust_decimal::Decimal)],
) -> Markup {
 html! {
 @for (pid, code, name, req_qty, min_pack, stock) in rows {
 tr oninput="omUpdateMaterialJson()" {
 td {
 div { (name) " " span class="text-muted text-xs" { "(" (code) ")" } }
 input type="hidden" name="m_product_id" value=(pid);
 span class="pack-hint text-[11px] text-muted" data-min-pack=(min_pack.map(|m| m.to_string()).unwrap_or_default()) {
 "需 " (crate::utils::fmt_qty(*req_qty)) " / 库存 " (crate::utils::fmt_qty(*stock))
 @if let Some(mp) = min_pack { " / min_pack " (crate::utils::fmt_qty(*mp)) }
 }
 }
 td {
 input class="w-[100px] text-right px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
 type="number" step="any" name="m_planned_qty" value=(req_qty.to_string())
 data-min-pack=(min_pack.map(|m| m.to_string()).unwrap_or_default())
 oninput="omValidatePack(this); omUpdateMaterialJson()";
 }
 td {
 input class="w-[100px] text-right px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
 type="number" step="any" name="m_unit_cost" value="0"
 oninput="omUpdateMaterialJson()";
 }
 td class="line-subtotal font-mono tabular-nums text-right text-[13px]" { "0.00" }
 td {
 button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-surface transition-colors duration-150" title="删除"
 onclick="this.closest('tr').remove();omUpdateMaterialJson()" {
 (icon::x_icon("w-3.5 h-3.5"))
 }
 }
 }
 }
 }
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
 _="on submit if !omValidateAllPacks() then halt the event"
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
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="work_order_id"
 hx-get=(OmOutsourcingWoSummaryPath::PATH) hx-trigger="change" hx-target="#routing-zone" hx-swap="innerHTML" {
 option value="" { "请选择工单" }
 @for wo in work_orders {
 option value=(wo.id) { (wo.doc_number) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工序" }
 div id="routing-zone" {
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted outline-none" name="routing_id" disabled {
 option value="" { "先选择关联工单" }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "计划数量 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="planned_qty" required
 hx-get=(OmOutsourcingSuggestMaterialsPath::PATH) hx-trigger="change" hx-target="#material-tbody" hx-swap="innerHTML"
 hx-include="[name='routing_id'], [name='source_warehouse_id'], [name='work_order_id']";
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "单价 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="unit_price" required;
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
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "发料源仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="source_warehouse_id" required {
 option value="" { "请选择仓库" }
 @for w in warehouses {
 @if !w.is_virtual {
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
 div class="overflow-x-auto -mx-2" {
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
 div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
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
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" id="modal-planned-qty" required;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "单位成本" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" id="modal-unit-cost";
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
 '<td><input class="w-[100px] text-right px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="m_planned_qty" value="' + qty + '"></td>' +
 '<td><input class="w-[100px] text-right px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="m_unit_cost" value="' + cost + '"></td>' +
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

function omValidatePack(el) {
 var mp = parseFloat(el.dataset.minPack);
 var qty = parseFloat(el.value);
 var hint = el.closest('tr').querySelector('.pack-hint');
 if (mp && mp > 0 && qty && qty % mp !== 0) {
 el.style.borderColor = 'var(--danger, #f53f3f)';
 if (hint) { hint.textContent = '需 ' + mp + ' 的整数倍（当前 ' + qty + '）'; hint.style.color = 'var(--danger, #f53f3f)'; }
 return false;
 }
 if (hint) hint.style.color = '';
 return true;
}

function omValidateAllPacks() {
 var ok = true;
 document.querySelectorAll('#material-tbody input[name=m_planned_qty]').forEach(function(el) {
 if (!omValidatePack(el)) ok = false;
 });
 return ok;
}
 "#))
 }
 }
}
 }
