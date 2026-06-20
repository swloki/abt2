use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{
 CreateBankAccountReq, CreateContactReq, CreateSupplierReq, SupplierCategory,
};
use abt_core::master_data::supplier::SupplierService;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::supplier::{SupplierCreatePath, SupplierDetailPath, SupplierListPath};
use crate::utils::RequestContext;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct SupplierCreateForm {
 // Basic info
 pub name: String,
 pub short_name: Option<String>,
 pub category: i16,
 pub tax_number: Option<String>,
 pub lead_time_days: Option<i32>,
 pub payment_terms: Option<String>,
 pub currency: Option<String>,
 // Contact
 pub contact_name: Option<String>,
 pub contact_phone: Option<String>,
 pub contact_position: Option<String>,
 pub contact_email: Option<String>,
 // Bank account
 pub bank_name: Option<String>,
 pub account_name: Option<String>,
 pub account_number: Option<String>,
 pub is_default: Option<String>,
 // Other
 pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("SUPPLIER", "create")]
pub async fn get_supplier_create(
 _path: SupplierCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, .. } = ctx;


 let content = supplier_create_page();
 let page_html = admin_page(
 is_htmx,
 "新建供应商",
 &claims,
 "purchase",
 SupplierCreatePath::PATH,
 "主数据管理",
 Some("新建供应商"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SUPPLIER", "create")]
pub async fn post_supplier_create(
 _path: SupplierCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<SupplierCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_service();

 let category = SupplierCategory::from_i16(form.category)
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效的供应类别"))?;

 let create_req = CreateSupplierReq {
 supplier_name: form.name,
 short_name: form.short_name.filter(|s| !s.is_empty()),
 category,
 tax_number: form.tax_number.filter(|s| !s.is_empty()),
 lead_time_days: form.lead_time_days,
 payment_terms: form.payment_terms.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 currency: form.currency.filter(|s| !s.is_empty()),
 };
 let supplier_id = svc.create(&service_ctx, &mut conn, create_req).await?;
 // Add contact if provided
 if let Some(contact_name) = form.contact_name.filter(|s| !s.is_empty()) {
 let contact_req = CreateContactReq {
 contact_name,
 phone: form.contact_phone.filter(|s| !s.is_empty()),
 email: form.contact_email.filter(|s| !s.is_empty()),
 position: form.contact_position.filter(|s| !s.is_empty()),
 is_primary: true,
 };
 svc.add_contact(&service_ctx, &mut conn, supplier_id, contact_req)
 .await?;
 }

 // Add bank account if provided
 if let (Some(bank_name), Some(account_name), Some(account_number)) = (
 form.bank_name.filter(|s| !s.is_empty()),
 form.account_name.filter(|s| !s.is_empty()),
 form.account_number.filter(|s| !s.is_empty()),
 ) {
 let bank_req = CreateBankAccountReq {
 bank_name,
 account_name,
 account_number,
 is_default: form.is_default.is_some(),
 };
 svc.add_bank_account(&service_ctx, &mut conn, supplier_id, bank_req)
 .await?;
 }

 let redirect = SupplierDetailPath { id: supplier_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn supplier_create_page() -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", SupplierListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回供应商列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建供应商" }
 }

 form id="supplier-form"
 hx-post=(SupplierCreatePath::PATH)
 hx-swap="none" {

 // ── Section: 基本信息 ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "供应商名称 " span class="text-danger" { "*" } }
 input type="text" name="name" required placeholder="请输入供应商名称" {}
 }
 div class="form-field" {
 label { "简称" }
 input type="text" name="short_name" placeholder="请输入简称" {}
 }
 div class="form-field" {
 label { "编码" }
 input type="text" value="自动生成" readonly
 class="bg-surface text-muted" {}
 }
 div class="form-field" {
 label { "供应类别 " span class="text-danger" { "*" } }
 select name="category" required {
 option value="" disabled selected { "-- 请选择 --" }
 option value="1" { "原材料" }
 option value="2" { "包装材料" }
 option value="3" { "外协加工" }
 option value="4" { "辅料耗材" }
 option value="5" { "服务" }
 }
 }
 div class="form-field" {
 label { "统一社会信用代码" }
 input type="text" name="tax_number" placeholder="请输入统一社会信用代码" {}
 }
 div class="form-field" {
 label { "交货天数 " span class="text-danger" { "*" } }
 input type="number" step="any" name="lead_time_days" required placeholder="请输入交货天数" {}
 }
 div class="form-field" {
 label { "付款条件" }
 select name="payment_terms" {
 option value="" { "-- 请选择 --" }
 option value="30天净额" { "30天净额" }
 option value="60天净额" { "60天净额" }
 option value="预付30%" { "预付30%" }
 option value="货到付款" { "货到付款" }
 option value="月结30天" { "月结30天" }
 option value="月结60天" { "月结60天" }
 option value="月结90天" { "月结90天" }
 }
 }
 div class="form-field" {
 label { "结算货币" }
 select name="currency" {
 option value="CNY" selected { "CNY - 人民币" }
 option value="USD" { "USD - 美元" }
 option value="JPY" { "JPY - 日元" }
 option value="AUD" { "AUD - 澳元" }
 option value="EUR" { "EUR - 欧元" }
 }
 }
 }
 }

 // ── Section: 联系人信息 ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "联系人信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "联系人 " span class="text-danger" { "*" } }
 input type="text" name="contact_name" placeholder="请输入联系人姓名" {}
 }
 div class="form-field" {
 label { "职位" }
 input type="text" name="contact_position" placeholder="请输入职位" {}
 }
 div class="form-field" {
 label { "手机号码 " span class="text-danger" { "*" } }
 input type="tel" name="contact_phone" placeholder="请输入手机号码" {}
 }
 div class="form-field" {
 label { "邮箱" }
 input type="email" name="contact_email" placeholder="请输入邮箱地址" {}
 }
 }
 }

 // ── Section: 银行账户信息 ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "银行账户信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "开户银行 " span class="text-danger" { "*" } }
 input type="text" name="bank_name" placeholder="请输入开户银行" {}
 }
 div class="form-field" {
 label { "账户名称 " span class="text-danger" { "*" } }
 input type="text" name="account_name" placeholder="请输入账户名称" {}
 }
 div class="form-field" {
 label { "银行账号 " span class="text-danger" { "*" } }
 input type="text" name="account_number" placeholder="请输入银行账号" {}
 }
 div class="form-field flex items-end pb-1" {
 label class="flex items-center cursor-pointer m-0 gap-2" {
 input type="checkbox" name="is_default" value="true" {}
 "默认账户"
 }
 }
 }
 }

 // ── Section: 其他 ──
 div class="data-card" class="mb-4" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "其他" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field field-full" {
 label { "备注" }
 textarea name="remark" placeholder="请输入备注信息…"
 class="w-full resize-y" class="min-h-[80px]" {}
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", SupplierListPath::PATH)) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "保存供应商"
 }
 }
 }
 }
 }
}
