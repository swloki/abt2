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
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_order::model::WorkOrderFilter;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{
    OmOutsourcingCreatePath, OmOutsourcingDetailPath, OmOutsourcingListPath,
    OmOutsourcingSuggestMaterialsPath, OmOutsourcingWoSummaryPath,
    OmOutsourcingSearchWoPath,
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
 #[serde(default)]
 pub virtual_warehouse_id: i64,
 #[serde(default)]
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

 let content = create_page(
 &suppliers.items,
 &products.items,
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

// ── 共享：BOM 展开 → 发料行 HTML ──

async fn build_material_rows(
 state: &crate::state::AppState,
 ctx: &abt_core::shared::types::context::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 work_order_id: i64,
 routing_id: i64,
 planned_qty: rust_decimal::Decimal,
 source_warehouse_id: Option<i64>,
) -> Result<String, abt_core::shared::types::DomainError> {
 use abt_core::master_data::bom::BomQueryService;
 use abt_core::master_data::bom::model::BomNode;
 use abt_core::master_data::product::ProductService;
 use abt_core::master_data::product::model::AcquireChannel;
 use abt_core::mes::production_batch::ProductionBatchService;
 use abt_core::wms::stock_ledger::StockLedgerService;

 if planned_qty <= rust_decimal::Decimal::ZERO {
     return Ok(html! { tr { td colspan="3" class="text-center text-muted text-sm py-4" {
         "请先选择关联工单以加载计划数量" } } }.into_string());
 }

 // 1. 获取工序产出品
 let batch_svc = state.production_batch_service();
 let routings = batch_svc.list_routings(ctx, db, work_order_id).await?;
 let routing = routings.iter().find(|r| r.id == routing_id)
 .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
 let semi_product_id = routing.product_id
 .ok_or_else(|| abt_core::shared::types::DomainError::business_rule(
 "该工序未关联产出品，请先在工单工序维护设置产出品"
 ))?;

 // 2. 从工单的 BOM 快照（或实时 BOM）中获取所有节点
 let wo_svc = state.work_order_service();
 let wo = wo_svc.find_by_id(ctx, db, work_order_id).await?;
 let bom_nodes: Vec<BomNode> = if let Some(sid) = wo.bom_snapshot_id {
     let snap = state.bom_query_service().get_snapshot_by_id(ctx, db, sid).await?
         .ok_or_else(|| abt_core::shared::types::DomainError::not_found("BOM snapshot"))?;
     snap.bom_detail.nodes
 } else {
     // 回退：按工单产品编码找 BOM
     let product_svc = state.product_service();
     let wo_product = product_svc.get_by_ids(ctx, db, vec![wo.product_id]).await?
         .into_iter().next()
         .ok_or_else(|| abt_core::shared::types::DomainError::not_found("Product"))?;
     let bom_id = state.bom_query_service().find_published_bom_by_product_code(ctx, db, &wo_product.product_code).await?
         .ok_or_else(|| abt_core::shared::types::DomainError::not_found(
             &format!("工单产品「{}」无 BOM", wo_product.product_code)))?;
     // 加载 BOM 节点
     let bom = state.bom_query_service().get(ctx, db, bom_id).await?;
     bom.bom_detail.nodes
 };

 // 3. 在 BOM 树中找到工序产出品节点，递归收集子物料
 let product_svc = state.product_service();
 let all_product_ids: Vec<i64> = bom_nodes.iter().map(|n| n.product_id).collect();
 let all_products = product_svc.get_by_ids(ctx, db, all_product_ids).await?;
 let product_map: std::collections::HashMap<i64, (AcquireChannel, String, String, Option<rust_decimal::Decimal>)> = all_products.iter()
     .map(|p| (p.product_id, (p.acquire_channel, p.product_code.clone(), p.pdt_name.clone(), p.min_pack_qty)))
     .collect();

 // 收集 semi_product_id 的直接子节点（仅下一级）
 let semi_node_id = bom_nodes.iter()
     .find(|n| n.product_id == semi_product_id)
     .map(|n| n.id);
 let child_ids: Vec<i64> = if let Some(nid) = semi_node_id {
     bom_nodes.iter()
         .filter(|n| n.parent_id == nid)
         .map(|n| n.product_id)
         .collect()
 } else {
     Vec::new()
 };

 if child_ids.is_empty() {
     let (_, code, name, _) = product_map.get(&semi_product_id)
         .map(|(_, c, n, _)| (AcquireChannel::Purchased, c.clone(), n.clone(), None::<rust_decimal::Decimal>))
         .unwrap_or((AcquireChannel::Purchased, semi_product_id.to_string(), semi_product_id.to_string(), None));
     return Ok(html! { tr { td colspan="3" class="text-center text-muted text-sm py-4" {
         "产出品「" (name) "」(" (code) ") 在工单 BOM 中无子物料" } } }.into_string());
 }

 // 4. 过滤采购件，计算需求量
 let stock_svc = state.stock_ledger_service();
 let mut rows: Vec<(i64, String, String, rust_decimal::Decimal, Option<rust_decimal::Decimal>, rust_decimal::Decimal)> = Vec::new();
 for cid in &child_ids {
     let (ac, code, name, min_pack) = match product_map.get(cid) {
         Some(v) => v.clone(),
         None => continue,
     };
     if ac != AcquireChannel::Purchased { continue; } // 只发外购物料

     // 在 BOM 节点中找到该物料的 quantity 和 loss_rate（semi_node_id 的直接子节点）
     let bom_node = bom_nodes.iter()
         .find(|n| n.product_id == *cid && n.parent_id == semi_node_id.unwrap_or(0));
     let bom_qty = bom_node.map(|n| n.quantity).unwrap_or(rust_decimal::Decimal::ONE);
     let loss_rate = bom_node.map(|n| n.loss_rate).unwrap_or(rust_decimal::Decimal::ZERO);
     let required_qty = bom_qty * planned_qty * (rust_decimal::Decimal::ONE + loss_rate);
     let stock = stock_svc.query_available(ctx, db, *cid, source_warehouse_id).await.unwrap_or_default();
     rows.push((*cid, code.clone(), name.clone(), required_qty, min_pack, stock));
 }

 if rows.is_empty() {
     return Ok(html! { tr { td colspan="3" class="text-center text-muted text-sm py-4" {
         "产出品在工单 BOM 中的子物料均为自制件，无需发料" } } }.into_string());
 }

 Ok(material_rows_fragment(&rows).into_string())
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

 // 若仅一个委外工序，预加载发料明细（OOB swap）
 let mut oob_materials = String::new();
 let outsourced_count = s.routings.iter().filter(|r| r.is_outsourced).count();
 if outsourced_count == 1 {
     if let Some(r) = s.routings.iter().find(|r| r.is_outsourced) {
         if r.product_id.is_some() {
             if let Ok(rows) = build_material_rows(&state, &service_ctx, &mut conn, q.work_order_id, r.id, s.planned_qty, None).await {
                 oob_materials = format!(r#"<tbody id="material-tbody" hx-swap-oob="true">{rows}</tbody>"#);
             }
         }
     }
 }

 let mut html = routing_select_fragment(&s).into_string();
 html.push_str(&oob_materials);
 Ok(Html(html))
}

// ── 工单搜索选择器 ──

#[derive(Debug, Deserialize)]
pub struct SearchWoParams {
 pub doc_number: Option<String>,
 pub product_code: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub status: Option<i16>,
}

#[require_permission("OUTSOURCING", "read")]
pub async fn search_work_orders(
 _path: OmOutsourcingSearchWoPath,
 ctx: RequestContext,
 axum::extract::Query(p): axum::extract::Query<SearchWoParams>,
) -> Result<Html<String>> {
 use abt_core::mes::WorkOrderStatus;
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.work_order_service();
 let status = p.status.and_then(|s| {
     if s == -1 { None }
     else { WorkOrderStatus::from_i16(s) }
 });
 let result = svc.list(
     &service_ctx, &mut conn,
     WorkOrderFilter {
         status,
         keyword: p.doc_number.filter(|s| !s.is_empty()),
         product_code: p.product_code.filter(|s| !s.is_empty()),
         ..Default::default()
     },
     1, 30,
 ).await?;
 Ok(Html(work_order_search_results(&result.items).into_string()))
}

fn work_order_search_results(items: &[abt_core::mes::work_order::WorkOrder]) -> Markup {
 use abt_core::mes::WorkOrderStatus;
 let status_label = |s: &WorkOrderStatus| -> (&str, &str) {
     match s {
         WorkOrderStatus::Draft => ("草稿", "status-draft"),
         WorkOrderStatus::Planned => ("已计划", "status-neutral"),
         WorkOrderStatus::Released => ("已下达", "status-progress"),
         WorkOrderStatus::InProduction => ("进行中", "status-progress"),
         WorkOrderStatus::Closed => ("已关闭", "status-completed"),
         WorkOrderStatus::Cancelled => ("已取消", "status-neutral"),
     }
 };
 html! {
 @if items.is_empty() {
 div class="text-center text-muted text-sm py-4" { "无匹配工单" }
 } @else {
 @for wo in items {
 @let (sl, sc) = status_label(&wo.status);
 div class="flex items-center gap-3 px-3 py-2 hover:bg-surface cursor-pointer border-b border-border-soft last:border-b-0 transition-colors duration-100"
     _=(format!("on click put '{}' into #wo-id-hidden's value then put '{}' into #wo-display's value then trigger change on #wo-id-hidden", wo.id, wo.doc_number)) {
 div class="flex-1 min-w-0" {
     div class="text-sm font-medium text-fg truncate" { (wo.doc_number) }
     div class="text-xs text-muted" { "计划 " (wo.planned_qty) " · " (wo.scheduled_end.format("%Y-%m-%d").to_string()) }
 }
 span class=(format!("status-pill {}", crate::utils::status_color(sc))) { (sl) }
 }
 }
 }
 }
}

/// 渲染关联工序下拉 + 回填脚本。默认只列 is_outsourced=true，checkbox 切换全部。
/// 选工单后自动回填：产品名/ID、计划数量、交期、委外类型。
/// 选工序后自动回填：单价（从工序的 unit_price）。
fn routing_select_fragment(s: &abt_core::om::outsourcing_order::model::WorkOrderOutsourcingSummary) -> Markup {
 // 默认只列可委外工序
 let outsourced: Vec<_> = s.routings.iter().filter(|r| r.is_outsourced).collect();
 // JSON: [id, step_no, process_name, is_outsourced, unit_price]
 let all_json = serde_json::to_string(
 &s.routings.iter().map(|r| (
     r.id, r.step_no, r.process_name.clone(), r.is_outsourced,
     r.unit_price.map(|p| p.to_string()).unwrap_or_default(),
 )).collect::<Vec<_>>(),
 ).unwrap_or_else(|_| "[]".into());
 html! {
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
     name="routing_id" id="routing-select" {
 @if outsourced.is_empty() {
 option value="" { "（无可委外工序，勾选下方显示全部）" }
 } @else {
 option value="" { "请选择工序" }
 }
 @for r in &outsourced {
 option value=(r.id) data-name=(r.process_name) data-price=(r.unit_price.map(|p| p.to_string()).unwrap_or_default()) { (r.step_no) " - " (r.process_name) }
 }
 }
 label class="inline-flex items-center gap-1 text-xs text-muted mt-1 cursor-pointer" {
 input type="checkbox" id="show-all-routings" class="cursor-pointer accent-accent";
 "显示全部工序"
 }
 // 回填数据（选工单后由 JS 读取并写入表单字段）
 input type="hidden" id="wo-summary-data"
     data-pid=(s.product_id) data-pname=(s.product_name) data-pq=(s.planned_qty.to_string()) data-se=(s.scheduled_end.to_string());
 input type="hidden" id="routings-json" value=(all_json);
 (maud::PreEscaped("<script>omInitWorkOrderPicker();</script>"))
 }
}

// ── 联动：发料即时查询（BOM 展开 + 库存 + min_pack_qty）──

#[derive(Debug, Deserialize)]
pub struct SuggestQuery {
 pub work_order_id: i64,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub routing_id: Option<i64>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub planned_qty: Option<rust_decimal::Decimal>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub source_warehouse_id: Option<i64>,
}

#[require_permission("OUTSOURCING", "read")]
pub async fn suggest_materials(
 _path: OmOutsourcingSuggestMaterialsPath,
 ctx: RequestContext,
 axum::extract::Query(q): axum::extract::Query<SuggestQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let routing_id = q.routing_id
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("请先选择关联工序"))?;
 let planned_qty = q.planned_qty.unwrap_or(rust_decimal::Decimal::ZERO);
 let html = build_material_rows(&state, &service_ctx, &mut conn, q.work_order_id, routing_id, planned_qty, q.source_warehouse_id).await
 .unwrap_or_else(|e| format!("<tr><td colspan=\"5\" class=\"text-center text-muted text-sm py-4\">加载失败: {e}</td></tr>"));
 Ok(Html(html))
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
 _="on submit if not omValidateAllPacks() then halt the event"
 {
 // ── Section 1: 关联信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::link_icon("w-[18px] h-[18px]"))
 "关联信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field relative" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工单" }
 input type="hidden" name="work_order_id" id="wo-id-hidden" value=""
     hx-get=(OmOutsourcingWoSummaryPath::PATH) hx-trigger="change" hx-target="#routing-zone" hx-swap="innerHTML";
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer transition-all duration-150 focus:border-accent"
     type="text" id="wo-display" value="" readonly
     placeholder="点击搜索工单…"
     _="on click toggle .is-open on #wo-picker then if #wo-picker matches .is-open trigger focus on #wo-doc-search";
 div id="wo-picker" class="absolute top-full left-0 right-0 z-[100] mt-1 bg-bg border border-border rounded-lg shadow-lg hidden [&.is-open]:block" {
     div class="grid grid-cols-3 gap-2 p-2 border-b border-border-soft" {
         input id="wo-doc-search" name="doc_number"
             class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
             type="text" placeholder="工单号…"
             hx-get=(OmOutsourcingSearchWoPath::PATH) hx-trigger="keyup changed delay:250ms"
             hx-target="#wo-search-results" hx-swap="innerHTML"
             hx-include="#wo-code-search, #wo-status-filter";
         input id="wo-code-search" name="product_code"
             class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
             type="text" placeholder="产品编码…"
             hx-get=(OmOutsourcingSearchWoPath::PATH) hx-trigger="keyup changed delay:250ms"
             hx-target="#wo-search-results" hx-swap="innerHTML"
             hx-include="#wo-doc-search, #wo-status-filter";
         select id="wo-status-filter" name="status"
             class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
             hx-get=(OmOutsourcingSearchWoPath::PATH) hx-trigger="change"
             hx-target="#wo-search-results" hx-swap="innerHTML"
             hx-include="#wo-doc-search, #wo-code-search" {
             option value="-1" { "全部状态" }
             option value="1" { "草稿" }
             option value="2" { "已计划" }
             option value="3" { "已下达" }
             option value="6" { "进行中" }
             option value="4" { "已关闭" }
             option value="5" { "已取消" }
         }
     }
     div id="wo-search-results" class="max-h-[280px] overflow-y-auto"
         hx-get=(OmOutsourcingSearchWoPath::PATH) hx-trigger="intersect once"
         hx-swap="innerHTML" {
         div class="flex items-center justify-center text-muted p-6 text-sm" { "输入关键词搜索…" }
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
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品 " span class="required" { "*" } }
 input type="hidden" name="product_id" id="product-id-hidden" value="" required;
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-fg outline-none" type="text" id="product-name-display" value="" readonly placeholder="选择工单后自动加载";
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
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "委外类型 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="outsourcing_type" required {
 option value="" { "请选择委外类型" }
 option value="1" { "整体委外" }
 option value="2" { "工序委外" }
 option value="3" { "材料委外" }
 option value="4" { "返工委外" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "计划数量 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="planned_qty" required;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "单价 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="unit_price" required;
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "预计交期" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="scheduled_date";
 }
 }
 }

 // ── Section 3: 发料明细 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "发料明细"
 }
 // 监听 routingSelected 事件 → 自动加载发料明细
 div id="material-loader"
     hx-get=(OmOutsourcingSuggestMaterialsPath::PATH)
     hx-trigger="routingSelected"
     hx-target="#material-tbody"
     hx-swap="innerHTML"
     hx-include="[name='work_order_id'], [name='routing_id'], [name='planned_qty']" {}
 div class="overflow-x-auto -mx-2" {
 table class="data-table" {
 thead { tr {
 th { "物料" }
 th { "应发数量" }
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

 // ── Page-specific JS ──
 script src="/om-outsourcing-create.js" {}
 }
}
 }
