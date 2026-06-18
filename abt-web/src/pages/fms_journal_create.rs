use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use crate::components::icon;
use serde::Deserialize;

use abt_core::fms::cash_journal::model::CreateCashJournalReq;
use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::enums::{CashDirection, CounterpartyType, JournalType};
use abt_core::shared::enums::document_type::DocumentType;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{JournalCreatePath, JournalListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct JournalCreateForm {
 pub journal_type: i16,
 pub direction: i16,
 pub amount: String,
 pub bank_account: String,
 pub counterparty_type: i16,
 pub counterparty_name: Option<String>,
 pub source_no: Option<String>,
 pub transaction_date: String,
 pub period: String,
 pub remark: String,
}

// ── Handlers ──

#[require_permission("FMS", "create")]
pub async fn get_create(
 _path: JournalCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, .. } = ctx;

 let content = journal_create_page();
 let page_html = admin_page(
 is_htmx,
 "新建日记账",
 &claims,
 "finance",
 JournalCreatePath::PATH,
 "财务管理",
 Some(JournalListPath::PATH),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

#[require_permission("FMS", "create")]
pub async fn create(
 _path: JournalCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<JournalCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let journal_type = JournalType::from_i16(form.journal_type)
 .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效日记账类型".into()))?;
 let direction = CashDirection::from_i16(form.direction)
 .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效方向".into()))?;
 let counterparty_type = CounterpartyType::from_i16(form.counterparty_type)
 .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;

 let amount: rust_decimal::Decimal = form.amount.parse()
 .map_err(|_| abt_core::shared::types::DomainError::Validation("无效金额".into()))?;
 let transaction_date = chrono::NaiveDate::parse_from_str(&form.transaction_date, "%Y-%m-%d")
 .map_err(|_| abt_core::shared::types::DomainError::Validation("无效交易日期".into()))?;

 let req = CreateCashJournalReq {
 journal_type,
 direction,
 amount,
 counterparty: abt_core::fms::enums::CounterpartyRef::from_parts(counterparty_type, 0),
 source_type: DocumentType::CashJournal,
 source_id: 0,
 bank_account: form.bank_account,
 transaction_date,
 period: form.period,
 remark: form.remark,
 lines: vec![],
 };

 let svc = state.cash_journal_service();
 let _id = svc.create(&service_ctx, &mut conn, req).await?;

 Ok(
 axum::response::Response::builder()
 .header("HX-Redirect", JournalListPath::PATH)
 .body(axum::body::Body::empty())
 .unwrap(),
 )
}

// ── Page ──

fn journal_create_page() -> Markup {
 html! {
 div {
 // 返回链接
 a href=(format!("{}?restore=true", JournalListPath::PATH)) class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg mb-4" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回列表"
 }
 // 标题
 h1 class="text-xl font-bold text-fg tracking-tight mb-6" { "新建出纳日记账" }

 form id="journal-create-form" hx-post=(JournalCreatePath::PATH) hx-swap="none" {
 // ── 基本信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::clipboard_document_icon("w-4 h-4"))
 " 基本信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 // 日记账类型
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "日记账类型 "
 span class="text-danger" { "*" }
 }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="journal_type" required {
 option value="" disabled selected { "请选择类型" }
 option value="1" { "销售回款" }
 option value="2" { "采购付款" }
 option value="3" { "费用报销" }
 option value="4" { "工资支付" }
 option value="5" { "其他" }
 }
 }
 // 收付方向
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "收付方向 "
 span class="text-danger" { "*" }
 }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="direction" required {
 option value="1" { "流入 (Inflow)" }
 option value="2" { "流出 (Outflow)" }
 }
 }
 // 金额
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "金额 "
 span class="text-danger" { "*" }
 }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent font-mono text-right" type="number" name="amount" step="any" min="0" required placeholder="0.00";
 }
 // 银行账户
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "银行账户 "
 span class="text-danger" { "*" }
 }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="text" name="bank_account" required placeholder="银行账号";
 }
 // 往来方类型
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "往来方类型 "
 span class="text-danger" { "*" }
 }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="counterparty_type" required {
 option value="1" { "客户" }
 option value="2" { "供应商" }
 option value="3" { "员工" }
 option value="4" { "其他" }
 }
 }
 // 往来方
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "往来方 "
 span class="text-danger" { "*" }
 }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="text" name="counterparty_name" placeholder="搜索选择往来方…";
 }
 // 来源单据
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源单据" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="text" name="source_no" placeholder="关联来源单号（可选）";
 }
 // 交易日期
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "交易日期 "
 span class="text-danger" { "*" }
 }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="date" name="transaction_date" required;
 }
 // 期间
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
 "期间 "
 span class="text-danger" { "*" }
 }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="month" name="period" required;
 }
 // 备注（占满整行）
 div class="form-field field-full" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="remark" placeholder="填写备注信息…" rows="3" {}
 }
 }
 }
}
 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", JournalListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "保存草稿" }
 button type="submit" form="journal-create-form" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "提交"
 }
 }
 }
 }
 }
 }
