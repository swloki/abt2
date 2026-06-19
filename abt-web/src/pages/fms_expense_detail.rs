use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, PreEscaped, Markup};
use rust_decimal::Decimal;

use abt_core::fms::enums::{ExpenseStatus, ExpenseType};
use abt_core::fms::expense::{ExpenseReimbursementService, ExpenseReimbursementItem};
use abt_core::shared::identity::{DepartmentService, UserService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{ExpenseDetailPath, ExpenseListPath, ExpenseApprovePath, ExpensePayPath, ExpenseSubmitPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn expense_type_text(t: &ExpenseType) -> &'static str {
 match t {
 ExpenseType::Travel => "差旅",
 ExpenseType::Office => "办公",
 ExpenseType::Transport => "交通",
 ExpenseType::Meal => "餐饮",
 ExpenseType::Other => "其他",
 }
}

fn status_text(s: &ExpenseStatus) -> (&'static str, &'static str) {
 match s {
 ExpenseStatus::Draft => ("草稿", "status-draft"),
 ExpenseStatus::Submitted => ("已提交", "status-submitted"),
 ExpenseStatus::Approved => ("已审批", "status-active"),
 ExpenseStatus::Paid => ("已付款", "status-active"),
 ExpenseStatus::Cancelled => ("已取消", "status-inactive"),
 }
}

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_detail(path: ExpenseDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let svc = state.expense_service();
 let expense = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 // 解析申请人名称
 let user_svc = state.user_service();
 let applicant_name = user_svc
 .get_user(&service_ctx, &mut conn, expense.applicant_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // 解析操作人名称
 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, expense.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // 解析部门名称
 let dept_svc = state.department_service();
 let department_name = match expense.department_id {
 Some(id) => dept_svc
 .get_department(&service_ctx, &mut conn, id)
 .await
 .map(|d| d.department_name)
 .unwrap_or_else(|_| "—".into()),
 None => "—".into(),
 };

 let (s_text, s_class) = status_text(&expense.status);
 let remark_display = if expense.remark.is_empty() {
 maud::PreEscaped("<span style=\"color:var(--muted)\">—</span>".to_string())
 } else {
 maud::PreEscaped(format!("<span style=\"color:var(--muted)\">{}</span>", expense.remark))
 };

 let content = html! {

 // 返回链接
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" href=(format!("{}?restore=true", ExpenseListPath::PATH)) {
 (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>"#))
 " 返回列表"
 }

 // 详情头
 div class="flex items-center gap-4 mb-6" {
 h1 class="text-xl font-bold font-mono tabular-nums" { (expense.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(s_class))) { (s_text) }
 }

 // 报销信息卡片
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "报销信息" }
 div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))]" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "单号" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (expense.doc_number) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "申请人" }
 span class="text-sm text-fg font-semibold" { (applicant_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "所属部门" }
 span class="text-sm text-fg font-medium" { (department_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "报销日期" }
 span class="text-sm text-fg font-medium" { (expense.expense_date.format("%Y-%m-%d")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "报销金额" }
 span class="text-sm text-fg font-medium font-mono tabular-nums text-accent font-bold text-lg" {
 "¥" (format!("{:.2}", expense.total_amount))
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "当前状态" }
 span class="text-sm text-fg font-medium" {
 span class=(format!("status-pill {}", crate::utils::status_color(s_class))) { (s_text) }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "操作人" }
 span class="text-sm text-fg font-medium" { (operator_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "创建时间" }
 span class="text-[13px] text-fg font-medium font-mono tabular-nums" { (expense.created_at.format("%Y-%m-%d %H:%M:%S")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "版本号" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { "v" (expense.version) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "备注" }
 (remark_display)
 }
 }
 }

 // 状态流转按钮（按当前 status 条件渲染）
 div class="flex gap-3 mb-6" {
 @if expense.status == ExpenseStatus::Draft {
 a class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium hover:opacity-90 cursor-pointer transition-all duration-150"
   hx-post=(ExpenseSubmitPath { id: path.id }.to_string())
   { "提交审批" }
 } @else if expense.status == ExpenseStatus::Submitted {
 a class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-success text-white text-sm font-medium hover:opacity-90 cursor-pointer transition-all duration-150"
   hx-post=(ExpenseApprovePath { id: path.id }.to_string())
   { "审批通过" }
 } @else if expense.status == ExpenseStatus::Approved {
 a class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-warning text-white text-sm font-medium hover:opacity-90 cursor-pointer transition-all duration-150"
   hx-post=(ExpensePayPath { id: path.id }.to_string())
   { "付款" }
 }
 }

 // 费用明细卡片
 (items_card(&items, expense.total_amount))
 };

 let current_path = ExpenseDetailPath { id: path.id }.to_string();
 let html = admin_page(
 is_htmx,
 "报销单详情",
 &claims,
 "finance",
 &current_path,
 "财务管理",
 Some(ExpenseListPath::PATH),
 content, &nav_filter, );
 Ok(Html(html.into_string()))
}

#[require_permission("FMS", "update")]
pub async fn submit(path: ExpenseSubmitPath, ctx: RequestContext) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.expense_service();
 svc.submit(&service_ctx, &mut conn, path.id).await?;
 let redirect = ExpenseDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("FMS", "update")]
pub async fn approve(path: ExpenseApprovePath, ctx: RequestContext) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.expense_service();
 svc.approve(&service_ctx, &mut conn, path.id).await?;
 let redirect = ExpenseDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("FMS", "update")]
pub async fn pay(path: ExpensePayPath, ctx: RequestContext) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.expense_service();
 svc.generate_payment_journal(&service_ctx, &mut conn, path.id).await?;
 let redirect = ExpenseDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn items_card(items: &[ExpenseReimbursementItem], total: Decimal) -> Markup {
 html! {
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "费用明细" }
 @if items.is_empty() {
 p class="text-center text-muted p-6" {
 "暂无费用明细"
 }
 } @else {
 div class="overflow-x-auto" {
 table class="data-table" style="min-width:700px" {
 thead {
 tr {
 th { "费用类型" }
 th class="text-right" { "金额" }
 th { "说明" }
 th { "发票号" }
 th { "成本中心" }
 th { "利润中心" }
 }
 }
 tbody {
 @for item in items {
 (item_row(item))
 }
 }
 }
 }
 div class="flex items-center justify-end gap-6 mt-4 pt-4 border-t border-border-soft" {
 span class="text-xs text-muted" { "合计金额" }
 span class="text-lg font-bold font-mono tabular-nums text-accent" { "¥" (format!("{:.2}", total)) }
 }
 }
 }
 }
}

fn item_row(item: &ExpenseReimbursementItem) -> Markup {
 let type_label = expense_type_text(&item.expense_type);
 let receipt = item.receipt_no.as_deref().unwrap_or("—");
 let cost = item.cost_center.map(|id| format!("CC-{:03}", id)).unwrap_or_else(|| "—".into());
 let profit = item.profit_center.map(|id| format!("PC-{:03}", id)).unwrap_or_else(|| "—".into());

 html! {
 tr {
 td {
 span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium text-muted mr-1" { (type_label) }
 }
 td class="text-right font-semibold" { "¥" (format!("{:.2}", item.amount)) }
 td { (item.description) }
 td class="font-mono tabular-nums text-xs" { (receipt) }
 td { (cost) }
 td { (profit) }
 }
 }
}
