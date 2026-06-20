use axum::response::{Html, IntoResponse};
use std::collections::HashMap;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::om::enums::{OutsourcingStatus, OutsourcingType, TrackingNodeType};
use abt_core::om::outsourcing_order::OutsourcingOrderService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::om::outsourcing_tracking::OutsourcingTrackingService;
use abt_core::shared::identity::UserService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::types::pagination::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{
 OmOutsourcingDetailPath, OmOutsourcingListPath, OmOutsourcingSendPath,
 OmOutsourcingReceivePath, OmOutsourcingConvertPath, OmOutsourcingCancelPath,
 OmRecordNodePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: &OutsourcingStatus) -> (&'static str, &'static str) {
 match s {
 OutsourcingStatus::Draft => ("草稿", "status-draft"),
 OutsourcingStatus::Sent => ("已发出", "status-sent"),
 OutsourcingStatus::InProduction => ("生产中", "status-progress"),
 OutsourcingStatus::Delivered => ("已发货", "status-shipped"),
 OutsourcingStatus::Received => ("已收货", "status-received"),
 OutsourcingStatus::Closed => ("已关闭", "status-completed"),
 OutsourcingStatus::ConvertedToInternal => ("转自制", "status-confirmed"),
 OutsourcingStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

fn type_label(t: &OutsourcingType) -> &'static str {
 match t {
 OutsourcingType::Full => "整体委外",
 OutsourcingType::Process => "工序委外",
 OutsourcingType::Material => "物料委外",
 OutsourcingType::Rework => "返工委外",
 }
}

fn node_type_label(t: &TrackingNodeType) -> &'static str {
 match t {
 TrackingNodeType::SendMaterial => "发料",
 TrackingNodeType::CarrierPickup => "承运商取件",
 TrackingNodeType::SupplierReceived => "供应商收料",
 TrackingNodeType::InProduction => "生产中",
 TrackingNodeType::Shipped => "已发货",
 TrackingNodeType::IqcInspected => "IQC检验",
 TrackingNodeType::Warehoused => "已入库",
 }
}
fn format_amount(d: rust_decimal::Decimal) -> String {
 let f: f64 = d.try_into().unwrap_or(0.0);
 if f == 0.0 { return "0".to_string(); }
 let abs = f.abs();
 if abs >= 1_000_000.0 {
 format!("{:.1}M", f / 1_000_000.0)
 } else {
 let formatted = format!("{:.2}", f);
 let parts: Vec<&str> = formatted.split('.').collect();
 let int_str = parts[0];
 let mut result = String::new();
 for (i, c) in int_str.chars().rev().enumerate() {
 if i > 0 && i % 3 == 0 { result.insert(0, ','); }
 result.insert(0, c);
 }
 let dec = parts[1].trim_end_matches('0');
 if dec.is_empty() { result } else { format!("{result}.{dec}") }
 }
}

fn status_pill(label: &str, class: &str) -> Markup {
 html! { span class=(format!("status-pill {}", crate::utils::status_color(class))) { (label) } }
}

// ── Handlers ──

#[require_permission("OM", "read")]
pub async fn get_detail(
 path: OmOutsourcingDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let svc = state.outsourcing_order_service();
 let tracking_svc = state.outsourcing_tracking_service();
 let supplier_svc = state.supplier_service();
 let product_svc = state.product_service();
 let user_svc = state.user_service();

 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

 let supplier_name = supplier_svc
 .get(&service_ctx, &mut conn, order.supplier_id)
 .await
 .map(|s| s.name)
 .unwrap_or_else(|_| "未知供应商".into());

 let product_name = product_svc
 .get(&service_ctx, &mut conn, order.product_id)
 .await
 .map(|p| p.pdt_name)
 .unwrap_or_else(|_| "—".into());

 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, order.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let warehouse_name = state.warehouse_service()
 .get(&service_ctx, &mut conn, order.virtual_warehouse_id)
 .await
 .map(|w| w.name)
 .unwrap_or_else(|_| "—".into());

 // 关联工单：解析为工单号（而非原始 ID）
 let work_order_name = match order.work_order_id {
 Some(wid) => state
 .work_order_service()
 .find_by_id(&service_ctx, &mut conn, wid)
 .await
 .map(|wo| wo.doc_number)
 .unwrap_or_else(|_| "—".into()),
 None => "—".into(),
 };

 // Tracking nodes
 let tracking = tracking_svc
 .list_by_outsourcing(&service_ctx, &mut conn, path.id, PageParams::new(1, 100))
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 // 发料明细
 let materials = svc
 .list_materials(&service_ctx, &mut conn, path.id)
 .await
 .unwrap_or_default();

 // 收发记录（WMS 库存流水，来自关联的调拨单）
 let inventory_records = svc
 .list_inventory_records(&service_ctx, &mut conn, path.id)
 .await
 .unwrap_or_default();

 // 发料源仓名称
 let source_warehouse_name = match order.source_warehouse_id {
 Some(wid) => state
 .warehouse_service()
 .get(&service_ctx, &mut conn, wid)
 .await
 .map(|w| w.name)
 .unwrap_or_else(|_| "—".into()),
 None => "—".into(),
 };

 // 金额计算：在途物料金额 = Σ(已发−已收回)×成本；加工费 = 计划数量×单价
 let in_transit_amount: rust_decimal::Decimal = materials
 .iter()
 .map(|m| (m.sent_qty - m.returned_qty) * m.unit_cost)
 .sum();
 let processing_fee = order.planned_qty * order.unit_price;

 let content = detail_page(
 &order, &supplier_name, &product_name, &operator_name, &warehouse_name,
 &work_order_name, &source_warehouse_name, &tracking,
 &materials, &inventory_records, in_transit_amount, processing_fee,
 );

 let page_html = admin_page(
 is_htmx, "委外单详情", &claims, "outsourcing",
 &OmOutsourcingDetailPath { id: path.id }.to_string(),
 "委外管理", Some(OmOutsourcingListPath::PATH),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

#[require_permission("OM", "update")]
pub async fn send_order(
 path: OmOutsourcingSendPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ActionForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.outsourcing_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 svc.send(&service_ctx, &mut conn, abt_core::om::outsourcing_order::SendOutsourcingReq {
 id: path.id,
 expected_version: order.version,
 remark: form.remark,
 }).await?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
 .body(axum::body::Body::empty())
 .unwrap())
}

#[require_permission("OM", "update")]
pub async fn receive_order(
 path: OmOutsourcingReceivePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ReceiveForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.outsourcing_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let received_qty: Decimal = form.received_qty.parse()
 .map_err(|_| abt_core::shared::types::DomainError::validation("无效收货数量"))?;
 svc.receive(&service_ctx, &mut conn, abt_core::om::outsourcing_order::ReceiveOutsourcingReq {
 id: path.id,
 expected_version: order.version,
 received_qty,
 warehouse_id: form.warehouse_id,
 iqc_passed_qty: None,
 remark: form.remark,
 }).await?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
 .body(axum::body::Body::empty())
 .unwrap())
}

#[require_permission("OM", "update")]
pub async fn convert_to_internal(
 path: OmOutsourcingConvertPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ActionForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.outsourcing_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 svc.convert_to_internal(&service_ctx, &mut conn, abt_core::om::outsourcing_order::ConvertToInternalReq {
 id: path.id,
 expected_version: order.version,
 remark: form.remark,
 }).await?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
 .body(axum::body::Body::empty())
 .unwrap())
}

#[require_permission("OM", "update")]
pub async fn cancel_order(
 path: OmOutsourcingCancelPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ActionForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.outsourcing_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 svc.cancel(&service_ctx, &mut conn, abt_core::om::outsourcing_order::CancelOutsourcingReq {
 id: path.id,
 expected_version: order.version,
 remark: form.remark,
 }).await?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
 .body(axum::body::Body::empty())
 .unwrap())
}

#[require_permission("OM", "update")]
pub async fn record_node(
 path: OmRecordNodePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<RecordNodeForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let tracking_svc = state.outsourcing_tracking_service();
 let node_type = TrackingNodeType::from_i16(form.node_type)
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效节点类型"))?;
 tracking_svc.record_node(&service_ctx, &mut conn, abt_core::om::outsourcing_tracking::RecordNodeReq {
 outsourcing_id: path.id,
 node_type,
 tracked_at: None,
 remark: form.remark,
 }).await?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
 .body(axum::body::Body::empty())
 .unwrap())
}

// ── Forms ──

#[derive(Debug, Deserialize)]
pub struct ActionForm {
 pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveForm {
 pub received_qty: String,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub warehouse_id: Option<i64>,
 pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordNodeForm {
 pub node_type: i16,
 pub remark: Option<String>,
}

// ── Components ──

fn materials_section(
 materials: &[abt_core::om::outsourcing_order::model::OutsourcingMaterial],
 in_transit_amount: rust_decimal::Decimal,
 processing_fee: rust_decimal::Decimal,
) -> Markup {
 html! {
 div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
 div class="h-[3px] bg-[linear-gradient(90deg,var(--warn),var(--accent),#60a5fa)]" {}
 div class="flex items-center justify-between px-8 py-5 border-b border-border-soft" {
 div class="flex items-center gap-3" {
 div class="w-10 h-10 rounded-xl grid place-items-center bg-[linear-gradient(135deg,rgba(217,119,6,0.08),rgba(37,99,235,0.08))]" {
 (maud::PreEscaped(r#"<svg class="w-5 h-5 text-warn" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/></svg>"#))
 }
 span class="text-[18px] font-bold text-fg" { "发料明细" }
 span class="text-xs text-muted bg-surface px-2 py-0.5 rounded-full" { (format!("{} 项物料", materials.len())) }
 }
 }
 div class="overflow-x-auto px-8 pb-5" {
 table class="data-table" {
 thead { tr {
 th { "物料" }
 th class="text-right" { "应发数量" }
 th class="text-right" { "已发数量" }
 th class="text-right" { "已收回" }
 th class="text-right" { "在途数量" }
 th class="text-right" { "单位成本" }
 th class="text-right" { "小计" }
 }}
 tbody {
 @if materials.is_empty() {
 tr { td colspan="7" class="text-center text-muted py-8" { "暂无发料明细" } }
 } @else {
 @for m in materials {
 tr {
 td { span class="font-semibold text-fg" { "产品 #" (m.product_id) } }
 td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(m.planned_qty)) }
 td class="text-right font-mono tabular-nums text-success" { (crate::utils::fmt_qty(m.sent_qty)) }
 td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(m.returned_qty)) }
 td class="text-right font-mono tabular-nums text-warn font-semibold" { (crate::utils::fmt_qty(m.sent_qty - m.returned_qty)) }
 td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(m.unit_cost)) }
 td class="text-right font-mono tabular-nums font-bold" { (crate::utils::fmt_qty(m.sent_qty * m.unit_cost)) }
 }
 }
 }
 }
 }
 }
 div class="flex items-center justify-end gap-8 px-8 py-4 border-t border-border-soft bg-surface" {
 div class="flex flex-col items-end gap-0.5" {
 span class="text-xs text-muted font-semibold" { "在途物料金额" }
 span class="text-lg font-bold font-mono tabular-nums text-warn" { (crate::utils::fmt_qty(in_transit_amount)) }
 }
 div class="flex flex-col items-end gap-0.5" {
 span class="text-xs text-muted font-semibold" { "加工费" }
 span class="text-lg font-bold font-mono tabular-nums text-accent" { (crate::utils::fmt_qty(processing_fee)) }
 }
 }
 }
 }
}

fn transactions_section(
 records: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
) -> Markup {
 use abt_core::wms::enums::TransactionType;
 html! {
 div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
 div class="h-[3px] bg-[linear-gradient(90deg,var(--success),var(--accent),#60a5fa)]" {}
 div class="flex items-center justify-between px-8 py-5 border-b border-border-soft" {
 div class="flex items-center gap-3" {
 div class="w-10 h-10 rounded-xl grid place-items-center bg-[linear-gradient(135deg,rgba(22,163,74,0.08),rgba(37,99,235,0.08))]" {
 (maud::PreEscaped(r#"<svg class="w-5 h-5 text-success" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 7h12M8 12h12M8 17h12M4 7h.01M4 12h.01M4 17h.01"/></svg>"#))
 }
 span class="text-[18px] font-bold text-fg" { "收发记录" }
 span class="text-xs text-muted bg-surface px-2 py-0.5 rounded-full" { (format!("{} 条记录", records.len())) }
 }
 }
 div class="overflow-x-auto px-8 pb-5" {
 table class="data-table" {
 thead { tr {
 th { "时间" }
 th { "类型" }
 th { "物料" }
 th class="text-right" { "数量" }
 th { "仓库" }
 }}
 tbody {
 @if records.is_empty() {
 tr { td colspan="5" class="text-center text-muted py-8" { "暂无收发记录" } }
 } @else {
 @for r in records {
 @let (type_label, type_cls) = match r.transaction_type {
 TransactionType::Transfer => ("调拨", "status-sent"),
 _ => ("流水", "status-progress"),
 };
 tr {
 td class="font-mono tabular-nums text-muted text-[13px]" { (r.created_at.format("%Y-%m-%d %H:%M")) }
 td { span class={ "inline-flex items-center px-2 py-0.5 rounded-full text-[11px] " (type_cls) } { (type_label) } }
 td class="font-medium" { "产品 #" (r.product_id) }
 td class={ "text-right font-mono tabular-nums " (if r.quantity.is_zero() { "" } else if r.quantity > rust_decimal::Decimal::ZERO { "text-success" } else { "text-danger" }) } {
 (crate::utils::fmt_qty(r.quantity))
 }
 td class="text-muted text-[13px]" { "仓库 #" (r.warehouse_id) }
 }
 }
 }
 }
 }
 }
 }
 }
}

fn detail_page(
 order: &abt_core::om::outsourcing_order::OutsourcingOrder,
 supplier_name: &str,
 product_name: &str,
 operator_name: &str,
 warehouse_name: &str,
 work_order_name: &str,
 source_warehouse_name: &str,
 tracking: &[abt_core::om::outsourcing_tracking::OutsourcingTracking],
 materials: &[abt_core::om::outsourcing_order::model::OutsourcingMaterial],
 inventory_records: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
 in_transit_amount: rust_decimal::Decimal,
 processing_fee: rust_decimal::Decimal,
) -> Markup {
 let (sl, sc) = status_label(&order.status);
 let tl = type_label(&order.outsourcing_type);
 let type_tag_cls = match order.outsourcing_type {
 OutsourcingType::Full => "type-tag full",
 OutsourcingType::Process => "type-tag process",
 OutsourcingType::Material => "type-tag material",
 OutsourcingType::Rework => "type-tag rework",
 };

 // Progress ring calculation
 let pct: f64 = if order.planned_qty > Decimal::ZERO {
 let ratio = order.completed_qty / order.planned_qty;
 (ratio * Decimal::ONE_HUNDRED).to_string().parse::<f64>().unwrap_or(0.0).min(100.0)
 } else {
 0.0
 };
 let r: f64 = 22.0;
 let circumference = 2.0 * std::f64::consts::PI * r;
 let offset = circumference * (1.0 - pct / 100.0);

 // Build tracking set: which node types have been recorded
 let tracked_nodes: HashMap<TrackingNodeType, &abt_core::om::outsourcing_tracking::OutsourcingTracking> =
 tracking.iter().map(|t| (t.node_type, t)).collect();
 let all_node_types = [
 TrackingNodeType::SendMaterial,
 TrackingNodeType::CarrierPickup,
 TrackingNodeType::SupplierReceived,
 TrackingNodeType::InProduction,
 TrackingNodeType::Shipped,
 TrackingNodeType::IqcInspected,
 TrackingNodeType::Warehoused,
 ];
 let _completed_count = all_node_types.iter().filter(|nt| tracked_nodes.contains_key(nt)).count();
 let active_index = all_node_types.iter().position(|nt| !tracked_nodes.contains_key(nt)).unwrap_or(all_node_types.len());

 html! { div {
 // ── Back link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", OmOutsourcingListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回委外单列表"
 }

 // ═══ Detail Hero Card ═══
 div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
 div class="h-1 bg-[linear-gradient(90deg,var(--accent),#60a5fa,var(--accent))] bg-[length:200%_100%] animate-shimmer-bar" {}
 div class="px-10 py-8" {

 // Title + Actions
 div class="flex items-center justify-between" {
 div {
 div class="text-[24px] font-bold text-fg flex items-center gap-[14px]" {
 div class="doc-icon [&_svg]:w-[22px] [&_svg]:h-[22px] [&_svg]:text-accent" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4"/></svg>"#))
 }
 (order.doc_number)
 }
 div class="flex items-center gap-[10px]" {
 (status_pill(sl, sc))
 span class=(type_tag_cls) { (tl) }
 span class="text-xs text-muted" { "v" (order.version) }
 }
 }
 div class="flex gap-[8px] shrink-0" {
 // 发料发送（Draft → Sent）— 主操作
 @if order.status == OutsourcingStatus::Draft {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" _="on click add .is-open to #send-modal" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-[15px] h-[15px]"><path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/></svg>"#))
 "发料"
 }
 }
 // 记录节点：非终态可见
 @if !matches!(order.status, OutsourcingStatus::Closed | OutsourcingStatus::ConvertedToInternal | OutsourcingStatus::Cancelled) {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click add .is-open to #record-node-modal" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-[15px] h-[15px]"><circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/></svg>"#))
 "记录节点"
 }
 }
 // 收货登记：仅 Sent
 @if order.status == OutsourcingStatus::Sent {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click add .is-open to #receive-modal" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-[15px] h-[15px]"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
 "收货登记"
 }
 }
 // 转自制：Draft / Sent
 @if matches!(order.status, OutsourcingStatus::Draft | OutsourcingStatus::Sent) {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click add .is-open to #convert-modal" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-[15px] h-[15px]"><path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4"/></svg>"#))
 "转自制"
 }
 }
 // 取消：仅 Draft
 @if order.status == OutsourcingStatus::Draft {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs text-danger border-[rgba(220,38,38,0.3)]" _="on click add .is-open to #cancel-modal" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-[15px] h-[15px]"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>"#))
 "取消"
 }
 }
 }
 }

 // Info Split: Key fields + Progress ring
 div class="grid border-t border-border-soft mt-7 pt-6" {
 div {
 div class="grid grid-cols-3 gap-[20px 48px]" {
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "供应商" }
 span class="text-[15px] text-fg font-semibold" { (supplier_name) }
 }
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "产品" }
 span class="text-[15px] text-fg font-semibold" { (product_name) }
 }
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "关联工单" }
 span class="text-[15px] text-fg font-semibold" {
 (work_order_name)
 }
 }
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "关联工序" }
 span class="text-[15px] text-fg font-semibold" {
 (order.routing_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "虚拟仓库" }
 span class="text-[15px] text-fg font-semibold" { (warehouse_name) }
 }
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "发料源仓库" }
 span class="text-[15px] text-fg font-semibold" { (source_warehouse_name) }
 }
 div class="flex flex-col gap-[6px]" {
 span class="text-xs text-muted font-semibold" { "预计交期" }
 span class="text-[15px] text-fg font-semibold font-mono tabular-nums" {
 (order.scheduled_date.map(|d| d.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 }
 // Detail row — secondary meta
 div class="flex flex-wrap gap-x-6 gap-y-1 py-2 text-sm" {
 span class="flex gap-[6px] text-xs text-muted" { "计划数量 " strong class="font-mono tabular-nums" { (crate::utils::fmt_qty(order.planned_qty)) } }
 span class="flex gap-[6px] text-xs text-muted" { "完成数量 " strong class="font-mono tabular-nums" class="text-success" { (crate::utils::fmt_qty(order.completed_qty)) } }
 span class="flex gap-[6px] text-xs text-muted" { "单价 " strong class="font-mono tabular-nums" { (crate::utils::fmt_qty(order.unit_price)) } }
 span class="flex gap-[6px] text-xs text-muted" { "总金额 " strong class="font-mono tabular-nums" class="text-accent" { (format_amount(order.planned_qty * order.unit_price)) } }
 span class="flex gap-[6px] text-xs text-muted" { "创建人 " strong { (operator_name) } }
 span class="flex gap-[6px] text-xs text-muted" { "创建 " strong class="font-mono tabular-nums" { (order.created_at.format("%Y-%m-%d %H:%M")) } }
 span class="flex gap-[6px] text-xs text-muted" { "更新 " strong class="font-mono tabular-nums" { (order.updated_at.format("%Y-%m-%d %H:%M")) } }
 }
 }
 // Progress Ring
 div class="flex flex-col items-center justify-center border-l" {
 div class="flex flex-col items-center gap-[8px]" {
 div class="w-[56px] h-[56px] relative" {
 svg viewBox="0 0 56 56" {
 circle fill="none" stroke="var(--border-soft)" stroke-width="4" cx="28" cy="28" r="22";
 circle fill="none" stroke="var(--accent)" stroke-width="4" stroke-linecap="round" cx="28" cy="28" r="22"
 stroke-dasharray=(format!("{circumference:.1}"))
 stroke-dashoffset=(format!("{offset:.1}"));
 }
 span class="absolute inset-0 grid place-items-center text-[14px] font-bold text-accent font-mono" { (format!("{:.0}%", pct)) }
 }
 span class="text-xs text-muted font-medium" { "完成进度" }
 }
 }
 }

 // Remark inside hero
 @if !order.remark.is_empty() {
 div style="margin-top:20px;padding-top:16px;border-top:1px dashed var(--border-soft)" {
 span class="text-xs text-muted font-semibold" { "备注" }
 p class="text-fg-2 text-[13px]" style="margin-top:6px;line-height:1.6" { (&order.remark) }
 }
 }
 }
 }

 // ═══ Tracking Timeline ═══
 div class="bg-bg border border-border-soft rounded-xl relative overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
 div class="h-[3px] bg-[linear-gradient(90deg,var(--success),var(--accent),#60a5fa)]" {}
 div class="flex items-center justify-between px-10 pt-8 pb-4" {
 div class="text-[18px] font-bold text-fg flex items-center gap-[14px]" {
 div class="tracking-icon-wrap [&_svg]:w-5 [&_svg]:h-5 [&_svg]:text-success" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>"#))
 }
 "追踪节点"
 }
 div class="text-xs text-muted flex items-center gap-[8px]" {
 span class="hint-dot" {}
 (format!("实时追踪 · 7 个节点 · 当前第 {} 步", active_index + 1))
 }
 }
 div class="relative pl-11 before:content-[''] before:absolute before:left-[17px] before:top-[18px] before:bottom-[18px] before:w-0.5 before:rounded-sm before:bg-[linear-gradient(180deg,var(--success)_0%,var(--success)_38%,var(--accent)_50%,var(--border)_55%,var(--border-soft)_100%)] pb-8" {
 @for (i, nt) in all_node_types.iter().enumerate() {
 @let tracked = tracked_nodes.get(nt);
 @let is_completed = tracked.is_some();
 @let is_active = !is_completed && i > 0 && tracked_nodes.contains_key(&all_node_types[i - 1]);
 @let dot_cls = if is_completed { "track-dot completed" } else if is_active { "track-dot active" } else { "track-dot pending" };
 @let label = node_type_label(nt);

 div class="relative pb-6 last:pb-0" {
 div class=(dot_cls) {}
 div class=(if is_active { "track-content active-content" } else { "track-content" }) {
 div class="flex-1" {
 div class=(if is_active || is_completed { "track-label" } else { "track-label muted" }) {
 (label)
 @if is_active {
 span class="ml-2 text-[11px] font-medium text-accent px-2.5 py-0.5 rounded-full bg-[rgba(37,99,235,0.1)]" { "当前" }
 }
 }
 @if let Some(t) = tracked {
 @if let Some(at) = t.tracked_at {
 div class="text-xs text-muted" { (at.format("%Y-%m-%d %H:%M")) }
 }
 @if let Some(remark) = &t.remark {
 div class="text-xs text-fg-2 bg-bg rounded-sm inline-flex items-start gap-[8px]" { (remark) }
 }
 } @else {
 @if let Some(t) = tracked_nodes.get(&all_node_types[if i > 0 { i - 1 } else { 0 }]) {
 @if let Some(planned) = &t.planned_at {
 div class="text-xs text-muted" { "计划 " (planned.format("%m-%d")) }
 }
 }
 }
 }
 div class="track-status" {
 @if is_completed {
 (status_pill("已完成", "status-completed"))
 } @else if is_active {
 (status_pill("进行中", "status-progress"))
 } @else {
 span class="text-[11px] text-muted" { "待完成" }
 }
 }
 }
 }
 }
 }
 }

 // ═══ Transaction Records ═══
 @if !tracking.is_empty() {
 div class="bg-bg border border-border-soft rounded-xl overflow-hidden mb-7 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg px-8 py-5 border-b border-border-soft" {
 div class="section-icon-wrap [&_svg]:w-4.5 [&_svg]:h-4.5 [&_svg]:text-accent" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 7h12M8 12h12M8 17h12M4 7h.01M4 12h.01M4 17h.01"/></svg>"#))
 }
 "收发记录"
 span class="section-count" { (tracking.len()) " 条记录" }
 }
 div class="overflow-hidden" {
 div class="overflow-x-auto px-8 pb-5" {
 table class="data-table" class="w-full" {
 thead {
 tr {
 th { "时间" }
 th { "类型" }
 th { "描述" }
 th { "状态" }
 }
 }
 tbody {
 @for t in tracking {
 tr {
 td class="text-muted text-[13px]" {
 (t.tracked_at.map(|at| at.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".into()))
 }
 td { (node_type_label(&t.node_type)) }
 td { (t.remark.as_deref().unwrap_or("—")) }
 td {
 @if t.tracked_at.is_some() {
 (status_pill("已完成", "status-completed"))
 } @else {
 (status_pill("计划中", "status-draft"))
 }
 }
 }
 }
 }
 }
 }
 }
 }
 }

 // ═══ Modals ═══
 // All modals use hyperscript (`_=`) for open/close, matching prototype structure.
 // Backdrop click: `_="on click[me is event.target] remove .is-open"` on modal-overlay div.
 // Close buttons: `_="on click remove .is-open from #X-modal"` on the button.

 // ── Record Node Modal ──
 // ═══ 发料明细 ═══
 (materials_section(materials, in_transit_amount, processing_fee))

 // ═══ 收发记录 ═══
 (transactions_section(inventory_records))

 // ── Send Modal（发料 Draft → Sent）──
 div id="send-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" _="on click[me is event.target] remove .is-open" {
 div class="bg-bg rounded-xl flex flex-col overflow-hidden shadow-xl" style="width:520px" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="flex items-center gap-2" {
 (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/></svg>"#))
 "确认发料"
 }
 button class="btn btn-text inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" _="on click remove .is-open from #send-modal" { "✕" }
 }
 form hx-post=(OmOutsourcingSendPath { id: order.id }.to_string()) hx-swap="none"
 hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#send-modal').classList.remove('is-open');this.reset()}" {
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 div class="text-[13px] text-fg-2 rounded-md mb-6 px-5 py-4" style="background:linear-gradient(135deg,var(--accent-bg),rgba(37,99,235,0.06));border:1px solid rgba(37,99,235,0.08)" {
 "发料后系统将创建 WMS 库存调拨单，将发料明细从源仓调拨到委外虚拟仓（"
 strong class="text-fg" { (warehouse_name) }
 "），并记录 " strong class="text-accent" { "发料" } " 追踪节点，状态变为「已发出」。"
 }
 div class="form-field field-full" {
 label { "备注（可选）" }
 textarea name="remark" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] resize-y" rows="2" placeholder="发料备注…" {}
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click remove .is-open from #send-modal" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "确认发料" }
 }
 }
 }
 }

 div id="record-node-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" _="on click[me is event.target] remove .is-open" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" style="width:520px" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="flex items-center" class="gap-2" {
 (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/></svg>"#))
 "记录追踪节点"
 }
 button class="btn btn-text inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" _="on click remove .is-open from #record-node-modal" {
 "✕"
 }
 }
 form hx-post=(OmRecordNodePath { id: order.id }.to_string()) hx-swap="none"
 hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#record-node-modal').classList.remove('is-open');this.reset()}" {
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 div class="text-[13px] text-fg-2" class="rounded-md mb-6" class="px-5 py-4" style="background:linear-gradient(135deg,rgba(22,163,74,0.04),rgba(22,163,74,0.08));border:1px solid rgba(22,163,74,0.08)" {
 "当前已完成节点："
 strong class="text-success" {
 @if let Some(last) = tracking.last() {
 (node_type_label(&last.node_type))
 } @else {
 "无"
 }
 }
 "，下一可记录节点："
 strong class="text-accent" {
 @if let Some(last) = tracking.last() {
 @if let Some(next) = all_node_types.iter().find(|nt| nt.as_i16() > last.node_type.as_i16()) {
 (node_type_label(next))
 } @else {
 "已全部完成"
 }
 } @else {
 "发料"
 }
 }
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "节点类型" }
 select name="node_type" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" class="w-full" {
 @for nt in all_node_types.iter() {
 @let label = node_type_label(nt);
 option value=(nt.as_i16()) { (label) }
 }
 }
 }
 div class="form-field" {
 label { "实际时间" }
 input type="datetime-local" name="actual_time" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" class="w-full" {}
 }
 div class="form-field field-full" {
 label { "备注" }
 textarea name="remark" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" rows="2" placeholder="节点备注…" class="w-full resize-y" {}
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click remove .is-open from #record-node-modal" {
 "取消"
 }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "确认记录" }
 }
 }
 }
 }

 // ── Receive Modal ──
 div id="receive-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" _="on click[me is event.target] remove .is-open" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="flex items-center" class="gap-2" {
 (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
 "收货登记"
 }
 button class="btn btn-text inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" _="on click remove .is-open from #receive-modal" {
 "✕"
 }
 }
 form hx-post=(OmOutsourcingReceivePath { id: order.id }.to_string()) hx-swap="none"
 hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#receive-modal').classList.remove('is-open');this.reset()}" {
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 div class="text-[13px] text-fg-2" class="rounded-md mb-6" class="px-5 py-4" style="background:linear-gradient(135deg,var(--accent-bg),rgba(37,99,235,0.06));border:1px solid rgba(37,99,235,0.08)" {
 div class="flex items-center flex-wrap" class="gap-4" {
 span { "委外单 " strong class="text-fg" { (order.doc_number) } }
 span class="text-border" { "|" }
 span { (product_name) }
 span class="text-border" { "|" }
 span { (supplier_name) }
 span class="text-border" { "|" }
 span { "计划 " span class="font-mono tabular-nums" class="font-bold" { (order.planned_qty.to_string()) } " · 已收 " span class="font-mono tabular-nums text-success" class="font-bold" { (order.completed_qty.to_string()) } }
 }
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "本次收货数量 " span class="text-danger" { "*" } }
 input type="number" name="received_qty" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" placeholder="请输入数量" min="1" class="w-full" required {}
 }
 div class="form-field" {
 label { "入库仓库" }
 select name="warehouse_id" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" class="w-full" {
 option value="23332" selected { "成品仓（默认）" }
 }
 }
 div class="form-field" {
 label { "IQC 合格数量" }
 input type="number" name="qualified_qty" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" placeholder="自动填充" class="w-full" {}
 }
 div class="form-field" {
 label { "IQC 不合格数量" }
 input type="number" name="unqualified_qty" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" placeholder="0" class="w-full" {}
 }
 div class="form-field field-full" {
 label { "备注" }
 textarea name="remark" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" rows="2" placeholder="收货备注…" class="w-full resize-y" {}
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click remove .is-open from #receive-modal" {
 "取消"
 }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "确认收货" }
 }
 }
 }
 }

 // ── Convert Modal ──
 div id="convert-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" _="on click[me is event.target] remove .is-open" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" style="width:520px" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="flex items-center" class="gap-2" {
 (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--warn)" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><path d="M12 9v4M12 17h.01"/></svg>"#))
 "转自制确认"
 }
 button class="btn btn-text inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" _="on click remove .is-open from #convert-modal" {
 "✕"
 }
 }
 form hx-post=(OmOutsourcingConvertPath { id: order.id }.to_string()) hx-swap="none"
 hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#convert-modal').classList.remove('is-open');this.reset()}" {
 div class="overflow-y-auto flex-1 min-h-0 p-6" class="text-center" class="p-8" {
 div class="grid" class="w-16" style="height:64px;border-radius:50%;background:linear-gradient(135deg,rgba(217,119,6,0.08),rgba(217,119,6,0.15));place-items:center;margin:0 auto var(--space-5)" {
 (maud::PreEscaped(r#"<svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="var(--warn)" stroke-width="2"><path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4"/></svg>"#))
 }
 p class="font-bold text-fg" class="text-lg" style="margin:0 0 var(--space-2)" { "将委外单转为内部生产？" }
 p class="text-muted" class="text-sm" style="margin:0 0 var(--space-6);line-height:1.7" { "系统将自动创建新的内部工单，" br {} "并将已发物料从委外虚拟仓调回。" }
 div class="text-left" {
 div class="form-field" {
 label { "备注（可选）" }
 textarea name="remark" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" rows="2" placeholder="转自制原因…" class="w-full resize-y" {}
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click remove .is-open from #convert-modal" {
 "取消"
 }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" style="background:linear-gradient(135deg,var(--warn),#f59e0b)" { "确认转自制" }
 }
 }
 }
 }

 // ── Cancel Modal ──
 div id="cancel-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" _="on click[me is event.target] remove .is-open" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" style="width:480px" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="flex items-center" class="gap-2" {
 (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--danger)" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>"#))
 "取消委外单"
 }
 button class="btn btn-text inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative [&_svg]:w-4 [&_svg]:h-4" type="button" _="on click remove .is-open from #cancel-modal" {
 "✕"
 }
 }
 form hx-post=(OmOutsourcingCancelPath { id: order.id }.to_string()) hx-swap="none"
 hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#cancel-modal').classList.remove('is-open');this.reset()}" {
 div class="overflow-y-auto flex-1 min-h-0 p-6" class="text-center" class="p-8" {
 div class="grid" class="w-16" style="height:64px;border-radius:50%;background:linear-gradient(135deg,rgba(220,38,38,0.08),rgba(220,38,38,0.15));place-items:center;margin:0 auto var(--space-5)" {
 (maud::PreEscaped(r#"<svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="var(--danger)" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>"#))
 }
 p class="font-bold text-fg" class="text-lg" style="margin:0 0 var(--space-2)" { "确认取消此委外单？" }
 p class="text-muted" class="text-sm" style="margin:0 0 var(--space-6);line-height:1.7" { "仅草稿状态可取消。取消后不可恢复。" }
 div class="text-left" {
 div class="form-field" {
 label { "取消原因 " span class="text-danger" { "*" } }
 textarea name="remark" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" rows="2" placeholder="请填写取消原因…" class="w-full resize-y" required {}
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" _="on click remove .is-open from #cancel-modal" {
 "返回"
 }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90" { "确认取消" }
 }
 }
 }
 }
 }}
}
