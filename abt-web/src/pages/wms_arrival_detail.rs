use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::arrival_notice::model::{
 ArrivalNotice, InspectArrivalNoticeReq, InspectItemReq, ReceiveArrivalNoticeReq, ReceiveItemReq,
};
use abt_core::wms::arrival_notice::repo::ArrivalNoticeRepo;
use abt_core::wms::arrival_notice::ArrivalNoticeService;
use abt_core::wms::enums::ArrivalStatus;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_arrival::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct ArrivalActionForm {
 pub action: String,
}

// ── Status Label ──

fn status_label(s: ArrivalStatus) -> (&'static str, &'static str) {
 match s {
 ArrivalStatus::Draft => ("草稿", "status-draft"),
 ArrivalStatus::Received => ("已收货", "status-received"),
 ArrivalStatus::Inspecting => ("检验中", "status-inspecting"),
 ArrivalStatus::Accepted => ("已接收", "status-completed"),
 ArrivalStatus::PartiallyAccepted => ("部分接收", "status-partial"),
 ArrivalStatus::Rejected => ("已拒收", "status-danger"),
 ArrivalStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

// ── Workflow Steps ──

fn workflow_steps(status: ArrivalStatus) -> Markup {
 let steps: &[(&str, bool)] = &[
 ("草稿", true),
 ("已收货", false),
 ("检验中", false),
 ("全部接收", false),
 ];

 let completed: Vec<bool> = match status {
 ArrivalStatus::Draft => vec![false, false, false, false],
 ArrivalStatus::Received => vec![true, true, false, false],
 ArrivalStatus::Inspecting => vec![true, true, true, false],
 ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted | ArrivalStatus::Rejected => {
 vec![true, true, true, true]
 }
 ArrivalStatus::Cancelled => vec![true, false, false, false],
 };

 let current_idx = match status {
 ArrivalStatus::Draft => Some(0),
 ArrivalStatus::Received => Some(1),
 ArrivalStatus::Inspecting => Some(2),
 ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted | ArrivalStatus::Rejected => Some(3),
 ArrivalStatus::Cancelled => None,
 };

 html! {
 div class="flex items-center mt-6 mb-6" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
 div class=(format!("w-[48px] h-[2px] {}", if completed[i] { "bg-[#10b981]" } else { "bg-border" })) {}
 }
 @let (dot_cls, text_cls, ring_cls) = match current_idx {
 Some(ci) if ci == i => ("bg-[#2563eb]", "text-[#2563eb] font-semibold", "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]"),
 _ if completed[i] => ("bg-[#10b981]", "text-[#10b981]", ""),
 _ => ("bg-[#d1d5db]", "text-[#9ca3af]", ""),
 };
 div class="flex items-center gap-2 shrink-0" {
 span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
 span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
 }
 }
 }
 div class="flex items-center flex-wrap" class="gap-4" class="mt-3" {
 span class="text-xs text-muted" { "检验结果分支：" }
 span class="items-center text-xs text-success" class="inline-flex gap-1" { "● 全部接收 (Accepted)" }
 span class="items-center text-xs" class="text-warn" class="inline-flex gap-1" { "● 部分接收 (Partially Accepted)" }
 span class="items-center text-xs text-danger" class="inline-flex gap-1" { "● 拒收 (Rejected)" }
 span style="color:var(--border-soft)" { "|" }
 span class="items-center text-xs text-muted" class="inline-flex gap-1" { "仅草稿状态可取消 (Cancelled)" }
 }
 }
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_arrival_detail(
 path: ArrivalDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.arrival_notice_service();

 let notice = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = ArrivalNoticeRepo::get_items(&mut conn, path.id)
 .await
 .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

 let wh_name = state.warehouse_service()
 .get(&service_ctx, &mut conn, notice.warehouse_id)
 .await
 .map(|w| w.name)
 .unwrap_or_else(|_| "—".into());

 let supplier_name = state.supplier_service()
 .get(&service_ctx, &mut conn, notice.supplier_id)
 .await
 .map(|s| s.name)
 .unwrap_or_else(|_| "—".into());

 let operator_name = state.user_service()
 .get_user(&service_ctx, &mut conn, notice.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let product_svc = state.product_service();
 let mut product_info: std::collections::HashMap<i64, (String, String, String, String)> = std::collections::HashMap::new();
 for item in &items {
 if !product_info.contains_key(&item.product_id)
 && let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
 product_info.insert(item.product_id, (
 p.product_code,
 p.pdt_name,
 if p.meta.specification.is_empty() { "—".to_string() } else { p.meta.specification.clone() },
 if p.unit.is_empty() { "—".to_string() } else { p.unit.clone() },
 ));
 }
 }

 let detail_path = ArrivalDetailPath { id: path.id }.to_string();
 let content = arrival_detail_page(&notice, &items, &product_info, &detail_path, &wh_name, &supplier_name, &operator_name);
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 来料通知详情", notice.doc_number),
 &claims,
 "inventory",
 &detail_path,
 "库存管理",
 Some(&notice.doc_number),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
pub async fn post_arrival_action(
 path: ArrivalDetailPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ArrivalActionForm>,
) -> crate::errors::Result<axum::response::Response> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.arrival_notice_service();

 match form.action.as_str() {
 "cancel" => {
 svc.cancel(&service_ctx, &mut conn, path.id).await?;
 }
 "receive" => {
 // 快速收货：实收数量 = 申报数量
 let items = ArrivalNoticeRepo::get_items(&mut conn, path.id)
 .await
 .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
 let receive_items: Vec<ReceiveItemReq> = items.iter()
 .map(|item| ReceiveItemReq {
 item_id: item.id,
 received_qty: item.declared_qty,
 batch_no: None,
 })
 .collect();
 svc.receive(&service_ctx, &mut conn, ReceiveArrivalNoticeReq {
 id: path.id,
 items: receive_items,
 }).await?;
 }
 "inspect" => {
 // 快速检验：合格数量 = 实收数量（全部接收）
 let items = ArrivalNoticeRepo::get_items(&mut conn, path.id)
 .await
 .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
 let inspect_items: Vec<InspectItemReq> = items.iter()
 .map(|item| InspectItemReq {
 item_id: item.id,
 accepted_qty: item.received_qty,
 })
 .collect();
 svc.inspect(&service_ctx, &mut conn, InspectArrivalNoticeReq {
 id: path.id,
 items: inspect_items,
 }).await?;
 }
 _ => {}
 }

 let redirect_url = ArrivalDetailPath { id: path.id }.to_string();
 let mut resp = axum::response::Response::default();
 resp.headers_mut().insert(
 axum::http::HeaderName::from_static("hx-redirect"),
 redirect_url.parse().unwrap(),
 );

 Ok(resp)
}

// ── Components ──

fn arrival_detail_page(
 notice: &ArrivalNotice,
 items: &[abt_core::wms::arrival_notice::model::ArrivalNoticeItem],
 product_info: &std::collections::HashMap<i64, (String, String, String, String)>,
 detail_path: &str,
 wh_name: &str,
 supplier_name: &str,
 operator_name: &str,
) -> Markup {
 let (status_text, status_class) = status_label(notice.status);
 let is_inspecting = notice.status == ArrivalStatus::Inspecting;

 html! {
 div {
 // ── Back Link ──
 a href=(format!("{}?restore=true", ArrivalListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回来料通知列表"
 }
 // ── Detail Header（裸 flex，非 card）──
 div class="flex items-start justify-between mb-6" {
 div class="flex items-center gap-4" {
 h1 class="text-xl font-bold font-mono tabular-nums" { (notice.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 }
 // ── Workflow Steps ──
 (workflow_steps(notice.status))
 // ── Basic Info（info-card 样式）──
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "基本信息" }
 div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))]" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "单据编号" }
 span class="text-sm text-fg font-mono tabular-nums" { (notice.doc_number) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "来源采购单" }
 span class="text-sm text-fg font-mono tabular-nums" {
 (notice.purchase_order_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "供应商" }
 span class="text-sm text-fg" { (supplier_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "到货仓库" }
 span class="text-sm text-fg" { (wh_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "到货库区" }
 span class="text-sm text-fg" {
 (notice.zone_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "到货日期" }
 span class="text-sm text-fg font-mono tabular-nums" { (notice.arrival_date.format("%Y-%m-%d")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "送货单号" }
 span class="text-sm text-fg font-mono tabular-nums" { (notice.delivery_note.as_deref().unwrap_or("—")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "操作员" }
 span class="text-sm text-fg" { (operator_name) }
 }
 }
 }
 // ── 行项明细（data-card）──
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "行号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格" }
 th { "单位" }
 th class="text-right text-[13px]" { "申报数量" }
 th class="text-right text-[13px]" { "实收数量" }
 th class="text-right text-[13px]" { "合格数量" }
 th { "批次号" }
 }
 }
 tbody {
 @for (i, item) in items.iter().enumerate() {
 @let (code, name, spec, unit) = product_info.get(&item.product_id)
 .map(|(c, n, s, u)| (c.as_str(), n.as_str(), s.as_str(), u.as_str()))
 .unwrap_or(("—", "—", "—", "—"));
 tr {
 td class="font-mono tabular-nums" { (i + 1) }
 td class="font-mono tabular-nums" { (code) }
 td { (name) }
 td { (spec) }
 td { (unit) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.declared_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.received_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.accepted_qty)) }
 td class="font-mono tabular-nums" { (item.batch_no.as_deref().unwrap_or("—")) }
 }
 }
 @if items.is_empty() {
 tr {
 td colspan="9" class="text-center text-muted py-8" {
 "暂无物料明细"
 }
 }
 }
 }
 }
 }
 }
 // ── IQC 质检结果区 ──
 @if is_inspecting || notice.status == ArrivalStatus::Accepted || notice.status == ArrivalStatus::PartiallyAccepted || notice.status == ArrivalStatus::Rejected {
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)] [border-left:3px_solid_var(--warn)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft flex items-center gap-2" {
 (icon::clipboard_list_icon("w-[18px] h-[18px]"))
 "IQC质检结果"
 span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08] ml-2" { "检验中" }
 }
 div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))] mb-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "检验标准" }
 span class="text-sm text-fg" { "GB/T 2828.1 抽样检验" }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "AQL等级" }
 span class="text-sm text-fg font-mono tabular-nums" { "0.65" }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "检验员" }
 span class="text-sm text-fg" { "—" }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "计划完成日期" }
 span class="text-sm text-fg font-mono tabular-nums" { "—" }
 }
 }
 div class="rounded-sm px-4 py-3 text-sm text-fg-2" class="border border-border" style="background:var(--surface-warm)" {
 strong class="text-warn" { "⚠ IQC硬门规则：" }
 "质检不合格的物料将阻断入库流程。不合格批次将触发MRB（物料评审委员会）处理流程，需由质量部判定：退货 / 让步接收 / 挑选使用。"
 }
 }
 }
 // ── Action Bar ──
 div class="flex items-center gap-3 mt-6" {
 (arrival_action_buttons(notice.status, detail_path))
 }
 }
 }
}

fn arrival_action_buttons(status: ArrivalStatus, detail_path: &str) -> Markup {
 match status {
 ArrivalStatus::Draft => {
 html! {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-post=(detail_path)
 hx-vals=r#"{"action":"cancel"}"#
 hx-confirm="确定要取消此来料通知吗？"
 hx-redirect=(detail_path) {
 (icon::x_icon("w-4 h-4"))
 "取消"
 }
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(detail_path)
 hx-vals=r#"{"action":"receive"}"#
 hx-confirm="确定要确认收货吗？实收数量将自动按申报数量填写。"
 hx-redirect=(detail_path) {
 (icon::check_circle_icon("w-4 h-4"))
 "确认收货"
 }
 }
 }
 ArrivalStatus::Received => {
 html! {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-post=(detail_path)
 hx-vals=r#"{"action":"cancel"}"#
 hx-confirm="确定要取消此来料通知吗？"
 hx-redirect=(detail_path) {
 (icon::x_icon("w-4 h-4"))
 "取消"
 }
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(detail_path)
 hx-vals=r#"{"action":"inspect"}"#
 hx-confirm="确定要开始检验并确认接收吗？合格数量将按实收数量自动填写。"
 hx-redirect=(detail_path) {
 (icon::clipboard_list_icon("w-4 h-4"))
 "检验接收"
 }
 }
 }
 _ => html! {},
 }
}
