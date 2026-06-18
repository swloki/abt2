use std::collections::HashMap;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::{InvoiceStatus, PurchaseOrderStatus};
use abt_core::purchase::order::model::*;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::payment_schedule::PaymentScheduleService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

const DECIMAL_100: Decimal = Decimal::from_parts(100, 0, 0, false, 0);
// ── Helpers ──

fn fmt_pct_str(qty: Decimal, total: Decimal) -> String {
    if total <= Decimal::ZERO {
        return "0%".into();
    }
    let v = (qty / total * DECIMAL_100).round_dp(1);
    let s = v.to_string();
    if s.ends_with(".0") {
        format!("{}%", &s[..s.len() - 2])
    } else {
        format!("{}%", s)
    }
}

fn fmt_pct_style(qty: Decimal, total: Decimal) -> String {
    if total <= Decimal::ZERO {
        return "width:0%".into();
    }
    format!("width:{}%", (qty / total * DECIMAL_100).round_dp(1))
}

fn status_label(s: PurchaseOrderStatus) -> (&'static str, &'static str) {
 match s {
 PurchaseOrderStatus::Draft => ("草稿", "status-draft"),
 PurchaseOrderStatus::Confirmed => ("已确认", "status-confirmed"),
 PurchaseOrderStatus::PartiallyReceived => ("部分收货", "status-partial"),
 PurchaseOrderStatus::Received => ("已收货", "status-shipped"),
 PurchaseOrderStatus::Closed => ("已关闭", "status-completed"),
 PurchaseOrderStatus::Cancelled => ("已取消", "status-cancelled"),
 PurchaseOrderStatus::PendingApproval => ("待审批", "status-pending"),
 }
}

fn invoice_status_label(s: InvoiceStatus) -> (&'static str, &'static str) {
 match s {
 InvoiceStatus::NoInvoice => ("未开票", "status-draft"),
 InvoiceStatus::ToInvoice => ("待开票", "status-pending"),
 InvoiceStatus::FullyInvoiced => ("已开票", "status-completed"),
 }
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_detail(
 path: PODetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();
 let supplier_svc = state.supplier_service();
 let product_svc = state.product_service();

 let schedules = state.payment_schedule_service()
 .list_by_order(&service_ctx, &mut conn, path.id)
 .await
 .unwrap_or_default();
 let user_svc = state.user_service();

 let order = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 let supplier_name = supplier_svc
 .get(&service_ctx, &mut conn, order.supplier_id)
 .await
 .map(|s| s.name)
 .unwrap_or_else(|_| "未知供应商".into());

 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, order.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let (product_names, product_codes, product_units, product_specs) = {
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 if product_ids.is_empty() {
 (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new())
 } else {
 let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
 let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
 let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
 let units: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.unit.clone())).collect();
 let specs: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.meta.specification.clone())).collect();
 (names, codes, units, specs)
 }
 };
 let content = po_detail_page(&order, &items, &schedules, &OrderDetailContext { supplier_name: &supplier_name, operator_name: &operator_name, product_names: &product_names, product_codes: &product_codes, product_units: &product_units, product_specs: &product_specs });
 let page_html = admin_page(
 is_htmx, "订单详情", &claims, "purchase",
 &format!("{}/{}", POListPath::PATH, path.id),
 "采购管理", Some("订单详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn confirm_po(
 path: POConfirmPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();

 svc.confirm(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = PODetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn cancel_po(
 path: POCancelPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();

 svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = PODetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn submit_po(
 axum::extract::Path(id): axum::extract::Path<i64>,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();
 svc.submit(&service_ctx, &mut conn, id, None).await?;
 let redirect = PODetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn approve_po_order(
 axum::extract::Path(id): axum::extract::Path<i64>,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();
 svc.approve_po(&service_ctx, &mut conn, id, None).await?;
 let redirect = PODetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn reject_po(
 axum::extract::Path(id): axum::extract::Path<i64>,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();
 svc.reject(&service_ctx, &mut conn, id, "退回修改".to_string(), None).await?;
 let redirect = PODetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct ItemChangesForm {
 pub changes_json: String,
}

#[derive(Debug, serde::Deserialize)]
struct ChangeItem {
 #[serde(rename = "type")]
 change_type: String,
 item_id: Option<i64>,
 product_id: Option<i64>,
 quantity: Option<String>,
 unit_price: Option<String>,
 description: Option<String>,
 discount_pct: Option<String>,
 tax_rate_id: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn update_po_items(
 axum::extract::Path(id): axum::extract::Path<i64>,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ItemChangesForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();

 let raw_changes: Vec<ChangeItem> = serde_json::from_str(&form.changes_json)
 .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效变更数据: {e}")))?;

 let changes: Vec<PoItemChange> = raw_changes.into_iter().filter_map(|c| {
 match c.change_type.as_str() {
 "add" => {
 let quantity: rust_decimal::Decimal = c.quantity?.parse().ok()?;
 let unit_price: rust_decimal::Decimal = c.unit_price?.parse().ok()?;
 Some(PoItemChange::AddItem(CreateOrderItemRequest {
 product_id: c.product_id?,
 line_no: 0,
 description: c.description.unwrap_or_default(),
 quantity,
 unit_price,
 quotation_item_id: None,
 expected_delivery_date: None,
 discount_pct: c.discount_pct.as_deref().and_then(|s| s.parse().ok()).unwrap_or(rust_decimal::Decimal::ZERO),
 tax_rate_id: c.tax_rate_id.as_deref().and_then(|s| s.parse().ok()).filter(|&v: &i64| v > 0),
 }))
 }
 "update" => {
 Some(PoItemChange::UpdateItem {
 item_id: c.item_id?,
 quantity: c.quantity.as_deref().and_then(|s| s.parse().ok()),
 unit_price: c.unit_price.as_deref().and_then(|s| s.parse().ok()),
 discount_pct: c.discount_pct.as_deref().and_then(|s| s.parse().ok()),
 tax_rate_id: c.tax_rate_id.as_deref().map(|s| s.parse().ok()).map(|opt| opt.filter(|&v: &i64| v > 0)),
 })
 }
 "remove" => Some(PoItemChange::RemoveItem { item_id: c.item_id? }),
 _ => None,
 }
 }).collect();

 svc.update_items_after_confirm(&service_ctx, &mut conn, id, changes, None).await?;

 let redirect = PODetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct MergeForm {
 pub order_ids: String, // comma-separated, e.g. "1,2,3"
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn merge_po(
 ctx: RequestContext,
 axum::Form(form): axum::Form<MergeForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();

 let order_ids: Vec<i64> = form.order_ids.split(',')
 .filter_map(|s| s.trim().parse().ok())
 .collect();

 let target_id = svc.merge_orders(&service_ctx, &mut conn, order_ids, None).await?;
 let redirect = PODetailPath { id: target_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: PurchaseOrderStatus) -> Markup {
 let steps: &[(&str, PurchaseOrderStatus)] = &[
 ("草稿", PurchaseOrderStatus::Draft),
 ("已确认", PurchaseOrderStatus::Confirmed),
 ("部分收货", PurchaseOrderStatus::PartiallyReceived),
 ("已收货", PurchaseOrderStatus::Received),
 ("已关闭", PurchaseOrderStatus::Closed),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == PurchaseOrderStatus::Cancelled;

 html! {
 div class="flex items-center mt-6 mb-6" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
 div class=(format!("w-[48px] h-[2px] {}", if i <= current_idx && !is_cancelled { "bg-[#10b981]" } else { "bg-border" })) {}
 }
 @let (dot_cls, text_cls, ring_cls) = if is_cancelled {
 ("bg-border-soft", "text-muted", "")
 } else if i < current_idx {
 ("bg-[#10b981]", "text-[#10b981]", "")
 } else if i == current_idx {
 ("bg-[#2563eb]", "text-[#2563eb] font-semibold", "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]")
 } else {
 ("bg-[#d1d5db]", "text-[#9ca3af]", "")
 };
 div class="flex items-center gap-2 shrink-0" {
 span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
 span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
 }
 }
 @if is_cancelled {
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 shrink-0" {
 span class="w-2.5 h-2.5 rounded-full shrink-0 bg-[#ef4444]" {}
 span class="text-xs text-[#ef4444] font-semibold whitespace-nowrap" { "已取消" }
 }
 }
 }
 }
}

// ── Components ──

struct OrderDetailContext<'a> {
 supplier_name: &'a str,
 operator_name: &'a str,
 product_names: &'a HashMap<i64, String>,
 product_codes: &'a HashMap<i64, String>,
 product_units: &'a HashMap<i64, String>,
 product_specs: &'a HashMap<i64, String>,
}

fn po_detail_page(
 order: &PurchaseOrder,
 items: &[PurchaseOrderItem],
 schedules: &[abt_core::purchase::payment_schedule::model::PaymentSchedule],
 ctx: &OrderDetailContext,
) -> Markup {
 let (status_text, status_class) = status_label(order.status);
 let expected_delivery = order.expected_delivery_date
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_else(|| "—".into());
 let payment_terms = order.payment_terms.as_deref().unwrap_or("—");
 let delivery_address = order.delivery_address.as_deref().unwrap_or("—");
 let received_total: Decimal = items.iter()
 .map(|i| i.received_qty * i.unit_price)
 .sum();
 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" href=(format!("{}?restore=true", POListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回采购订单列表"
 }
 // ── Detail Header（裸 flex，非 card）──
 div class="flex items-start justify-between mb-6" {
 div class="flex items-center gap-4" {
 h1 class="text-xl font-bold font-mono tabular-nums" { (order.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 @let (inv_text, inv_class) = invoice_status_label(order.invoice_status);
 span class=(format!("status-pill {}", crate::utils::status_color(inv_class))) { (inv_text) }
 }
 div class="flex gap-3" {
 @if order.status == PurchaseOrderStatus::Draft {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(POEditPath { id: order.id }.to_string()) {
 (icon::edit_icon("w-4 h-4"))
 "编辑"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(format!("/admin/purchase/orders/{}/submit", order.id))
 hx-confirm="提交审批？" {
 "提交审批"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-post=(POConfirmPath { id: order.id }.to_string())
 hx-confirm="确认此订单？确认后将通知供应商。" {
 (icon::check_circle_icon("w-4 h-4"))
 "直接确认"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-white border-none hover:opacity-90 text-sm font-medium cursor-pointer transition-all duration-150"
 hx-post=(POCancelPath { id: order.id }.to_string())
 hx-confirm="确认取消此订单？取消后不可恢复。" {
 "取消订单"
 }
 }
 @if order.status == PurchaseOrderStatus::PendingApproval {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(format!("/admin/purchase/orders/{}/approve", order.id))
 hx-confirm="审批通过？" {
 "审批通过"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-white border-none hover:opacity-90 text-sm font-medium cursor-pointer transition-all duration-150"
 hx-post=(format!("/admin/purchase/orders/{}/reject", order.id)) {
 "退回修改"
 }
 }
 }
 }
 // ── Workflow Steps ──
 (workflow_steps(order.status))
 // ── Fulfillment Progress ──
 @if !items.is_empty() {
 @let total_qty: Decimal = items.iter().map(|i| i.quantity).sum();
 @let received_qty: Decimal = items.iter().map(|i| i.received_qty).sum();
 @let inspected_qty: Decimal = items.iter().map(|i| i.inspected_qty).sum();
 @let returned_qty: Decimal = items.iter().map(|i| i.returned_qty).sum();
 @let pending_qty = total_qty - received_qty - returned_qty;
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 // Header: 标题 + 统计
 div class="flex items-center justify-between mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg" {
 (icon::chart_bar_icon("w-4 h-4 text-accent"))
 "履约进度"
 }
 div class="flex gap-6" {
 div class="text-center" {
 div class="text-lg font-bold font-mono tabular-nums text-success" { (crate::utils::fmt_qty(received_qty)) }
 div class="text-[11px] text-muted mt-0.5" { "已收货" }
 }
 div class="text-center" {
 div class="text-lg font-bold font-mono tabular-nums text-accent" { (crate::utils::fmt_qty(inspected_qty)) }
 div class="text-[11px] text-muted mt-0.5" { "已检验" }
 }
 div class="text-center" {
 div class="text-lg font-bold font-mono tabular-nums text-danger" { (crate::utils::fmt_qty(returned_qty)) }
 div class="text-[11px] text-muted mt-0.5" { "已退货" }
 }
 div class="text-center" {
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(pending_qty)) }
 div class="text-[11px] text-muted mt-0.5" { "待收货" }
 }
 }
 }
 // 细进度条
 div class="flex h-2 rounded overflow-hidden bg-border-soft" {
 @if received_qty > Decimal::ZERO {
 div class="bg-success [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(fmt_pct_style(received_qty, total_qty)) {}
 }
 @if inspected_qty > Decimal::ZERO {
 div class="bg-accent [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(fmt_pct_style(inspected_qty, total_qty)) {}
 }
 @if returned_qty > Decimal::ZERO {
 div class="bg-danger [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(fmt_pct_style(returned_qty, total_qty)) {}
 }
 @if pending_qty > Decimal::ZERO {
 div class="bg-border [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(fmt_pct_style(pending_qty, total_qty)) {}
 }
 }
 // 图例
 div class="flex gap-5 mt-3 flex-wrap" {
 span class="flex items-center gap-1.5 text-[11px] text-muted" {
 span class="w-2 h-2 rounded-full shrink-0 bg-success" {}
 (format!("已收货 {}", fmt_pct_str(received_qty, total_qty)))
 }
 span class="flex items-center gap-1.5 text-[11px] text-muted" {
 span class="w-2 h-2 rounded-full shrink-0 bg-accent" {}
 (format!("已检验 {}", fmt_pct_str(inspected_qty, total_qty)))
 }
 span class="flex items-center gap-1.5 text-[11px] text-muted" {
 span class="w-2 h-2 rounded-full shrink-0 bg-danger" {}
 (format!("已退货 {}", fmt_pct_str(returned_qty, total_qty)))
 }
 span class="flex items-center gap-1.5 text-[11px] text-muted" {
 span class="w-2 h-2 rounded-full shrink-0 bg-border" {}
 (format!("待收货 {}", fmt_pct_str(pending_qty, total_qty)))
 }
 }
 }
 }
 // ── Order Info（info-card 样式）──
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "订单信息" }
 div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))]" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "供应商" }
 span class="text-sm text-fg" { (ctx.supplier_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "订单日期" }
 span class="text-sm text-fg font-mono tabular-nums" { (order.order_date.format("%Y-%m-%d")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "预计到货" }
 span class="text-sm text-fg font-mono tabular-nums" { (expected_delivery) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "付款条款" }
 span class="text-sm text-fg" { (payment_terms) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "交货地址" }
 span class="text-sm text-fg" { (delivery_address) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "币种" }
 span class="text-sm text-fg" { "CNY" }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "采购员" }
 span class="text-sm text-fg" { (ctx.operator_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "关联报价" }
 span class="text-sm text-fg" { "—" }
 }
 }
 }
 // ── Items Table（data-card）──
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "行号" }
 th { "物料编码" }
 th { "物料名称" }
 th { "规格" }
 th { "单位" }
 th class="text-right text-[13px]" { "数量" }
 th class="text-right text-[13px]" { "单价" }
 th class="text-right text-[13px]" { "金额" }
 th class="text-right text-[13px]" { "已收货" }
 th class="text-right text-[13px]" { "已检验" }
 th class="text-right text-[13px]" { "已退货" }
 th { "预计到货" }
 }
 }
 tbody {
 @for item in items {
 (item_row(item, ctx.product_names, ctx.product_codes, ctx.product_units, ctx.product_specs))
 }
 @if items.is_empty() {
 tr {
 td colspan="12" class="text-center text-muted py-8" {
 "暂无明细"
 }
 }
 }
 }
 }
 }
 // ── Amount Summary ──
 div class="flex justify-end gap-8 p-5 [border-top:1px_solid_var(--border-soft)] bg-surface-raised" {
 div class="flex gap-3" {
 span class="text-[11px] text-muted font-medium uppercase" { "订单总额" }
 span class="text-[20px] font-bold text-accent" { (format!("¥ {:.2}", order.total_amount)) }
 }
 div class="flex gap-3" {
 span class="text-[11px] text-muted font-medium uppercase" { "已收货金额" }
 span class="text-[20px] font-bold text-fg" { (format!("¥ {:.2}", received_total)) }
 }
 }
 }
 // ── Remarks（info-card 样式）──
 @if !order.remark.is_empty() {
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "备注" }
 p class="text-sm text-muted" { (order.remark.as_str()) }
 }
 }
 // ── Payment Schedule（info-card 样式）──
 @if !schedules.is_empty() {
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "付款计划" }
 table class="data-table" {
 thead {
 tr {
 th { "期次" }
 th { "到期日" }
 th class="text-right text-[13px]" { "百分比" }
 th class="text-right text-[13px]" { "应付金额" }
 th class="text-right text-[13px]" { "已付金额" }
 }
 }
 tbody {
 @for (i, sched) in schedules.iter().enumerate() {
 tr {
 td class="font-mono tabular-nums" { (i + 1) }
 td class="font-mono tabular-nums" { (sched.due_date.format("%Y-%m-%d").to_string()) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{}%", sched.payment_pct)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (sched.payment_amount) }
 td class="text-right text-[13px] font-mono tabular-nums" { (sched.paid_amount) }
 }
 }
 }
 }
 }
 }
 }
 }
}

fn item_row(
 item: &PurchaseOrderItem,
 names: &HashMap<i64, String>,
 codes: &HashMap<i64, String>,
 units: &HashMap<i64, String>,
 specs: &HashMap<i64, String>,
) -> Markup {
 let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let unit = units.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let spec = specs.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let expected_delivery = item.expected_delivery_date
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_else(|| "—".into());
 html! {
 tr {
 td class="font-mono tabular-nums" { (item.line_no) }
 td class="font-mono tabular-nums" { (product_code) }
 td { (product_name) }
 td { (spec) }
 td { (unit) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.quantity)) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.unit_price)) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.amount)) }
 td class="text-right text-[13px]" { (if item.received_qty > Decimal::ZERO { format!("{:.2}", item.received_qty) } else { "—".into() }) }
 td class="text-right text-[13px]" { (if item.inspected_qty > Decimal::ZERO { format!("{:.2}", item.inspected_qty) } else { "—".into() }) }
 td class="text-right text-[13px]" { (if item.returned_qty > Decimal::ZERO { format!("{:.2}", item.returned_qty) } else { "—".into() }) }
 td { (expected_delivery) }
 }
 }
}
