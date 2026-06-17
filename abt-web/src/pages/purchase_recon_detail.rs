use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseReconStatus;
use abt_core::purchase::reconciliation::model::*;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_reconciliation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PurchaseReconStatus) -> (&'static str, &'static str) {
 match s {
 PurchaseReconStatus::Draft => ("草稿", "status-draft"),
 PurchaseReconStatus::Confirmed => ("已确认", "status-confirmed"),
 PurchaseReconStatus::Settled => ("已结算", "status-completed"),
 }
}

// ── Handlers ──

#[require_permission("PURCHASE_RECON", "read")]
pub async fn get_precon_detail(
 path: PreconDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_delete = ctx.has_permission("PURCHASE_RECON", "delete").await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_reconciliation_service();
 let supplier_svc = state.supplier_service();
 let user_svc = state.user_service();

 let recon = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 let supplier_name = supplier_svc
 .get(&service_ctx, &mut conn, recon.supplier_id)
 .await
 .map(|s| s.name)
 .unwrap_or_else(|_| "未知供应商".into());

 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, recon.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let content = precon_detail_page(&recon, &items, &supplier_name, &operator_name, can_delete);
 let page_html = admin_page(
 is_htmx, "对账详情", &claims, "purchase",
 &format!("{}/{}", PreconListPath::PATH, path.id),
 "采购管理", Some("对账详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_RECON", "update")]
pub async fn confirm_precon(
 path: PreconConfirmPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_reconciliation_service();

 svc.confirm(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = PreconDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: PurchaseReconStatus) -> Markup {
 let steps: &[(&str, PurchaseReconStatus)] = &[
 ("草稿", PurchaseReconStatus::Draft),
 ("已确认", PurchaseReconStatus::Confirmed),
 ("已结算", PurchaseReconStatus::Settled),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);

 html! {
 div class="flex items-center" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
 @let line_class = if i <= current_idx { "wf-line completed" } else { "wf-line" };
 div class=(line_class) {}
 }
 @let step_class = if i < current_idx {
 "wf-step completed"
 } else if i == current_idx {
 "wf-step current"
 } else {
 "wf-step"
 };
 div class=(step_class) {
 span class="w-[10px] h-[10px] rounded-full bg-border" {}
 (label)
 }
 }
 }
 }
}

// ── Components ──

fn precon_detail_page(
 recon: &PurchaseReconciliation,
 items: &[PurchaseReconItem],
 supplier_name: &str,
 operator_name: &str,
 can_delete: bool,
) -> Markup {
 let (status_text, status_class) = status_label(recon.status);

 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PreconListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回采购对账列表"
 }

 // ── Detail Header ──
 div class="block bg-bg border border-border-soft rounded-lg p-6" {
 div {
 div class="flex items-center justify-between" {
 h1 class="text-2xl font-extrabold font-mono tabular-nums" { (recon.doc_number) }
 span class=(format!("status-pill {status_class}")) { (status_text) }
 }
 }
 div class="flex gap-3" {
 @if recon.status == PurchaseReconStatus::Draft {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(PreconConfirmPath { id: recon.id }.to_string())
 hx-confirm="确认对此对账单进行对账？确认后将不可修改。" {
 (icon::check_circle_icon("w-4 h-4"))
 "确认对账"
 }
 }
 @if recon.status == PurchaseReconStatus::Draft && can_delete {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90-ghost"
 hx-post=(format!("/purchase/reconciliation/{}", recon.id))
 hx-confirm="确认删除此对账单？删除后不可恢复。" {
 (icon::trash_icon("w-4 h-4"))
 "删除"
 }
 }
 }
 }

 (workflow_steps(recon.status))
 // ── Reconciliation Info ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "对账信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "供应商名称" }
 span class="text-sm text-fg font-medium" { (supplier_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "对账期间" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (recon.period) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "状态" }
 span class=(format!("status-pill {status_class}")) { (status_text) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "操作人" }
 span class="text-sm text-fg font-medium" { (operator_name) }
 }
 }
 }

 // ── Items Table ──
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "订单ID" }
 th { "订单明细ID" }
 th class="text-right text-[13px]" { "收货数量" }
 th class="text-right text-[13px]" { "退货数量" }
 th class="text-right text-[13px]" { "退货冲减" }
 th class="text-right text-[13px]" { "单价" }
 th class="text-right text-[13px]" { "金额" }
 th { "已确认" }
 }
 }
 tbody {
 @for item in items {
 (item_row(item))
 }
 @if items.is_empty() {
 tr {
 td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
 "暂无明细"
 }
 }
 }
 }
 }
 }
 }

 // ── Amount Summary ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-top:var(--space-6)" {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "金额汇总" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "总金额" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (format!("{:.2}", recon.total_amount)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "确认金额" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (format!("{:.2}", recon.confirmed_amount)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "差异" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (format!("{:.2}", recon.difference)) }
 }
 }
 }

 // ── Remarks ──
 @if !recon.remark.is_empty() {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-top:var(--space-6)" {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "备注" }
 p class="text-muted" { (recon.remark.as_str()) }
 }
 }
 }
 }
}

fn item_row(item: &PurchaseReconItem) -> Markup {
 let confirmed = if item.confirmed { "✓" } else { "—" };

 html! {
 tr {
 td class="font-mono tabular-nums" { (item.order_id) }
 td class="font-mono tabular-nums" { (item.order_item_id) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.received_qty)) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.returned_qty)) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.returned_amount)) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.unit_price)) }
 td class="text-right text-[13px]" { (format!("{:.2}", item.amount)) }
 td style="text-align:center" { (confirmed) }
 }
 }
}
