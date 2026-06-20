use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::shared::identity::UserService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::enums::PaymentMethod;
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::payment::model::*;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::purchase::reconciliation::model::PurchaseReconciliationQuery;
use abt_core::shared::types::PageParams;
use abt_core::shared::types::DomainError;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::payment_request::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct PayCreateForm {
 pub supplier_id: i64,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub reconciliation_id: Option<i64>,
 pub payment_date: String,
 pub amount: String,
 pub payment_method: i16,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub bank_account_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub invoice_number: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub invoice_amount: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub remark: Option<String>,
}

// ── HTMX query params ──

#[derive(Debug, Deserialize)]
pub struct SupplierInfoParams {
 pub supplier_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("PAYMENT_REQUEST", "create")]
pub async fn get_pay_create(
 _path: PayCreatePath,
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
 let recon_svc = state.purchase_reconciliation_service();
 let user_svc = state.user_service();

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

 let reconciliations = recon_svc
 .list(
 &service_ctx,
 &mut conn,
 PurchaseReconciliationQuery::default(),
 PageParams::new(1, 200),
 )
 .await?;

 let applicant_name = user_svc
 .get_user(&service_ctx, &mut conn, claims.sub)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| claims.display_name.clone());

 let content = pay_create_page(
 &suppliers.items,
 &reconciliations.items,
 &applicant_name,
 );
 let page_html = admin_page(
 is_htmx,
 "新建付款申请",
 &claims,
 "purchase",
 PayCreatePath::PATH,
 "采购管理",
 Some("新建付款申请"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: return supplier info card (replaces entire supplier data-card) when supplier is selected.
/// Uses outerHTML swap so the contact/phone fields are updated along with bank account info.
#[require_permission("PAYMENT_REQUEST", "create")]
pub async fn get_supplier_info(
 ctx: RequestContext,
 axum::extract::Query(params): axum::extract::Query<SupplierInfoParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let supplier_svc = state.supplier_service();

 // Re-fetch supplier list for the dropdown
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

 let (contacts, bank_accounts) = if let Some(sid) = params.supplier_id {
 let contacts = supplier_svc
 .list_contacts(&service_ctx, &mut conn, sid)
 .await
 .unwrap_or_default();
 let bank_accounts = supplier_svc
 .list_bank_accounts(&service_ctx, &mut conn, sid)
 .await
 .unwrap_or_default();
 (contacts, bank_accounts)
 } else {
 (vec![], vec![])
 };

 let fragment = supplier_section(&suppliers.items, params.supplier_id, &contacts, &bank_accounts);
 Ok(Html(fragment.into_string()))
}

#[require_permission("PAYMENT_REQUEST", "create")]
pub async fn create_pay(
 _path: PayCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PayCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.payment_request_service();

 let payment_date = chrono::NaiveDate::parse_from_str(&form.payment_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效付款日期格式: {e}")))?;

 let amount: rust_decimal::Decimal = form
 .amount
 .parse()
 .map_err(|e| DomainError::validation(format!("无效金额格式: {e}")))?;

 let payment_method = PaymentMethod::from_i16(form.payment_method)
 .ok_or_else(|| DomainError::validation("无效付款方式".to_string()))?;

 let invoice_amount = form
 .invoice_amount
 .and_then(|s| s.parse::<rust_decimal::Decimal>().ok());

 let create_req = CreatePaymentRequestRequest {
 supplier_id: form.supplier_id,
 reconciliation_id: form.reconciliation_id,
 payment_date,
 amount,
 payment_method,
 bank_account_id: form.bank_account_id,
 invoice_number: form.invoice_number,
 invoice_amount,
 remark: form.remark.unwrap_or_default(),
 };

 let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

 let redirect = PayDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pay_create_page(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 reconciliations: &[abt_core::purchase::reconciliation::model::PurchaseReconciliation],
 applicant_name: &str,
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();

 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PayListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回付款申请列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建付款申请" }
 }

 form id="pay-form"
 hx-post=(PayCreatePath::PATH)
 hx-swap="none" {

 // ── 供应商信息 ──
 (supplier_section(suppliers, None, &[], &[]))

 // ── 付款信息 ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "付款信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "关联对账单" }
 select name="reconciliation_id" {
 option value="" { "无" }
 @for r in reconciliations {
 option value=(r.id) { (r.doc_number) " — " (r.period) }
 }
 }
 }
 div class="form-field" {
 label { "付款日期" span class="text-danger" { "*" } }
 input type="date" name="payment_date" value=(today) required {}
 }
 div class="form-field" {
 label { "付款金额" span class="text-danger" { "*" } }
 input type="number" step="any" id="pay-amount" name="amount" placeholder="0.00" required {}
 }
 div class="form-field" {
 label { "付款方式" span class="text-danger" { "*" } }
 select name="payment_method" required {
 option value="1" selected { "银行转账" }
 option value="2" { "现金" }
 option value="3" { "票据" }
 }
 }
 div class="form-field" {
 label { "发票号" }
 input type="text" name="invoice_number" placeholder="输入发票号" {}
 }
 div class="form-field" {
 label { "发票金额" }
 input type="number" step="any" name="invoice_amount" placeholder="0.00" {}
 }
 div class="form-field" {
 label { "申请人" }
 input type="text" value=(applicant_name) readonly {}
 }
 div class="form-field col-span-2" {
 label { "备注" }
 textarea name="remark" placeholder="输入付款申请相关备注信息…" class="w-full resize-y" class="rounded-sm" class="min-h-[80px] border border-border text-sm" style="padding:8px 12px;font-family:inherit" {}
 }
 }
 }

 // ── 三单匹配校验 ──
 div class="data-card" class="mb-4" {
 div class="flex items-center justify-between mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" class="m-0 p-0 border-none" { "三单匹配校验" }
 span class="items-center font-semibold" class="rounded-sm" class="inline-flex text-xs" style="gap:6px;padding:4px 12px;background:#fef9c3;color:#a16207;border:1px solid #fde68a" {
 (icon::clock_icon("w-3.5 h-3.5"))
 "待验证"
 }
 }
 div class="grid" class="grid grid-cols-3 gap-4" {
 // 验收单
 div class="flex items-center bg-surface" class="gap-3 rounded-sm" class="px-4 py-3 border border-border-soft" {
 (icon::check_circle_icon("w-5 h-5"))
 div {
 div class="text-sm font-semibold text-fg" { "验收单" }
 div class="text-success" class="text-xs" { "已匹配" }
 }
 }
 // 发票
 div class="flex items-center bg-surface" class="gap-3 rounded-sm" class="px-4 py-3 border border-border-soft" {
 (icon::clock_icon("w-5 h-5"))
 div {
 div class="text-sm font-semibold text-fg" { "发票" }
 div class="text-xs" style="color:#d97706" { "待验证" }
 }
 }
 // 对账单
 div class="flex items-center bg-surface" class="gap-3 rounded-sm" class="px-4 py-3 border border-border-soft" {
 (icon::check_circle_icon("w-5 h-5"))
 div {
 div class="text-sm font-semibold text-fg" { "对账单" }
 div class="text-success" class="text-xs" { "已匹配" }
 }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", PayListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "保存草稿" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "提交付款申请" }
 }
 }
 }
 }
 }
}

/// Renders the entire supplier info data-card.
/// On HTMX supplier change, this replaces the card via outerHTML swap so all
/// auto-filled fields (contact, phone, bank account) are updated atomically.
fn supplier_section(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 selected_supplier_id: Option<i64>,
 contacts: &[abt_core::master_data::supplier::model::SupplierContact],
 bank_accounts: &[abt_core::master_data::supplier::model::SupplierBankAccount],
) -> Markup {
 let primary_contact = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());
 let contact_name = primary_contact.map(|c| c.name.as_str()).unwrap_or("");
 let contact_phone = primary_contact.and_then(|c| c.phone.as_deref()).unwrap_or("");
 let default_account = bank_accounts.first();

 html! {
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "供应商信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "供应商" span class="text-danger" { "*" } }
 select name="supplier_id" required
 hx-get=(PaySupplierInfoPath::PATH)
 hx-trigger="change"
 hx-target="closest .data-card"
 hx-swap="outerHTML"
 hx-include="this" {
 option value="" disabled[selected_supplier_id.is_none()] { "请选择供应商" }
 @for s in suppliers {
 @let sel = selected_supplier_id == Some(s.id);
 option value=(s.id) selected[sel] { (s.name) }
 }
 }
 }
 div class="form-field" {
 label { "联系人" }
 input type="text" value=(contact_name) placeholder="自动填充" readonly {}
 }
 div class="form-field" {
 label { "联系电话" }
 input type="text" value=(contact_phone) placeholder="自动填充" readonly {}
 }
 }
 // 收款账户 info
 div class="mt-4" {
 div class="form-field" {
 label { "收款账户" }
 @if let Some(acct) = default_account {
 input type="text" value=(format!("{} — {} {}", acct.bank_name, acct.account_name, acct.account_number)) readonly {}
 input type="hidden" name="bank_account_id" value=(acct.id) {}
 div class="bg-surface text-muted grid" class="p-3 rounded-sm gap-4" class="mt-2 border border-border-soft text-xs" style="grid-template-columns:1fr 1fr 1fr" {
 div {
 span class="block font-medium text-fg" { "户名" }
 span { (acct.account_name) }
 }
 div {
 span class="block font-medium text-fg" { "开户行" }
 span { (acct.bank_name) }
 }
 div {
 span class="block font-medium text-fg" { "账号" }
 span class="font-mono" { (acct.account_number) }
 }
 }
 } @else if selected_supplier_id.is_some() {
 input type="text" value="" placeholder="该供应商暂无银行账户信息" readonly {}
 input type="hidden" name="bank_account_id" value="" {}
 } @else {
 input type="text" value="" placeholder="选择供应商后自动填充" readonly {}
 input type="hidden" name="bank_account_id" value="" {}
 }
 }
 }
 }
 }
}
