use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::purchase::enums::{PaymentMethod, PaymentStatus};
use abt_core::purchase::payment::model::*;
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::payment_request::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PaymentStatus) -> (&'static str, &'static str) {
 match s {
 PaymentStatus::Draft => ("草稿", "status-draft"),
 PaymentStatus::Approved => ("已审批", "status-confirmed"),
 PaymentStatus::Paid => ("已支付", "status-completed"),
 PaymentStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

fn payment_method_label(m: PaymentMethod) -> &'static str {
 match m {
 PaymentMethod::BankTransfer => "银行转账",
 PaymentMethod::Cash => "现金",
 PaymentMethod::Note => "票据",
 }
}


// ── Workflow Steps ──

fn workflow_steps(current: PaymentStatus) -> Markup {
 let steps: &[(&str, PaymentStatus)] = &[
 ("草稿", PaymentStatus::Draft),
 ("已审批", PaymentStatus::Approved),
 ("已付款", PaymentStatus::Paid),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == PaymentStatus::Cancelled;

 html! {
 div class="flex items-center mt-6 mb-6" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
 div class=(format!("w-[48px] h-[2px] {}", if i <= current_idx && !is_cancelled { "bg-success" } else { "bg-border" })) {}
 }
 @let (dot_cls, text_cls, ring_cls) = if is_cancelled {
 ("bg-border-soft", "text-muted", "")
 } else if i < current_idx {
 ("bg-success", "text-success", "")
 } else if i == current_idx {
 ("bg-accent", "text-accent font-semibold", "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]")
 } else {
 ("bg-slate-300", "text-slate-400", "")
 };
 div class="flex items-center gap-2 shrink-0" {
 span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
 span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
 }
 }
 @if is_cancelled {
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 shrink-0" {
 span class="w-2.5 h-2.5 rounded-full shrink-0 bg-danger-500" {}
 span class="text-xs text-danger-500 font-semibold whitespace-nowrap" { "已取消" }
 }
 }
 }
 }
}

// ── Handlers ──

#[require_permission("PAYMENT_REQUEST", "read")]
pub async fn get_pay_detail(
 path: PayDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.payment_request_service();
 let supplier_svc = state.supplier_service();
 let recon_svc = state.purchase_reconciliation_service();
 let user_svc = state.user_service();

 let pay = svc.get(&service_ctx, &mut conn, path.id).await?;

 let supplier_name = supplier_svc
 .get(&service_ctx, &mut conn, pay.supplier_id)
 .await
 .map(|s| s.name)
 .unwrap_or_else(|_| "未知供应商".into());

 let recon_doc_number = match pay.reconciliation_id {
 Some(rid) => recon_svc
 .get(&service_ctx, &mut conn, rid)
 .await
 .map(|r| r.doc_number)
 .ok(),
 None => None,
 };

 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, pay.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let content = pay_detail_page(&pay, &supplier_name, recon_doc_number.as_deref(), &operator_name);
 let page_html = admin_page(
 is_htmx, "付款详情", &claims, "purchase",
 &format!("{}/{}", PayListPath::PATH, path.id),
 "采购管理", Some("付款详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("PAYMENT_REQUEST", "update")]
pub async fn approve_pay(
 path: PayApprovePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.payment_request_service();

 svc.approve(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = PayDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PAYMENT_REQUEST", "update")]
pub async fn cancel_pay(
 path: PayCancelPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.payment_request_service();

 svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = PayDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pay_detail_page(
 pay: &PaymentRequest,
 supplier_name: &str,
 recon_doc_number: Option<&str>,
 operator_name: &str,
) -> Markup {
 let (status_text, status_class) = status_label(pay.status);

 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PayListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回付款列表"
 }

 // ── Detail Header ──
 div class="flex items-center justify-between bg-bg border border-border-soft rounded-lg p-6" {
 div {
 div class="flex items-center justify-between" {
 h1 class="text-2xl font-extrabold font-mono tabular-nums" { (pay.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 }
 div class="flex gap-3" {
 @if pay.status == PaymentStatus::Draft {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(PayApprovePath { id: pay.id }.to_string())
 hx-confirm="确认审批此付款申请？" {
 (icon::check_circle_icon("w-4 h-4"))
 "审批"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
 hx-post=(PayCancelPath { id: pay.id }.to_string())
 hx-confirm="确认取消此付款申请？取消后不可恢复。" {
 "取消"
 }
 }
 }
 }

 // ── Workflow Steps ──
 (workflow_steps(pay.status))

 // ── Payment Info ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "付款信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "供应商名称" }
 span class="text-sm text-fg font-medium" { (supplier_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "付款日期" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (pay.payment_date.format("%Y-%m-%d")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "付款方式" }
 span class="text-sm text-fg font-medium" { (payment_method_label(pay.payment_method)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "付款金额" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (format!("{}", pay.amount)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "关联对账单" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" {
 @if let Some(doc) = recon_doc_number {
 (doc)
 } @else {
 "—"
 }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "银行账户" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" {
 @if let Some(bank_id) = pay.bank_account_id {
 (format!("{}", bank_id))
 } @else {
 "—"
 }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "操作人" }
 span class="text-sm text-fg font-medium" { (operator_name) }
 }
 }
 }

 // ── Invoice Info ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 mt-6 shadow-[var(--shadow-sm)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "发票信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "发票号" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" {
 @if let Some(ref inv) = pay.invoice_number {
 (inv.as_str())
 } @else {
 "—"
 }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "发票金额" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" {
 @if let Some(amt) = pay.invoice_amount {
 (format!("{}", amt))
 } @else {
 "—"
 }
 }
 }
 }
 }

 // ── Remarks ──
 @if !pay.remark.is_empty() {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 mt-6 shadow-[var(--shadow-sm)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "备注" }
 p class="text-muted" { (pay.remark.as_str()) }
 }
 }
 }
 }
}
