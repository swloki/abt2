use chrono::Datelike;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::WorkReportService;
use abt_core::shared::identity::UserService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::WageListPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct WageListQuery {
 pub date_from: Option<String>,
 pub date_to: Option<String>,
}

#[require_permission("LABOR_COST", "read")]
pub async fn get_wage_list(
 _path: WageListPath, ctx: RequestContext,
 axum::extract::Query(query): axum::extract::Query<WageListQuery>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let report_svc = state.work_report_service();
 let wo_svc = state.work_order_service();
 let user_svc = state.user_service();

 // Default date range: current month
 let now = chrono::Local::now();
 let today = chrono::NaiveDate::parse_from_str(&now.format("%Y-%m-%d").to_string(), "%Y-%m-%d").unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
 let first_of_month = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
 let date_from = query.date_from.and_then(|d| d.parse().ok()).unwrap_or(first_of_month);
 let date_to = query.date_to.and_then(|d| d.parse().ok()).unwrap_or(today);

 let date_range = abt_core::mes::work_report::DateRange {
 from: date_from,
 to: date_to,
 };

 // Load all wage summaries
 let summaries = report_svc.list_all_wage_summaries(&service_ctx, &mut conn, date_range).await?;

 // Load worker names
 let users_result = user_svc.list_users(&service_ctx, &mut conn, 1, 200).await?;
 let user_map: HashMap<i64, String> = users_result.items.iter()
 .map(|u| (u.user_id, u.display_name.clone().unwrap_or_else(|| u.username.clone())))
 .collect();

 // Load work order doc numbers
 let mut wo_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
 for s in &summaries {
 for d in &s.details {
 wo_ids.insert(d.work_order_id);
 }
 }
 let mut wo_doc_map: HashMap<i64, String> = HashMap::new();
 for wo_id in wo_ids {
 if let Ok(wo) = wo_svc.find_by_id(&service_ctx, &mut conn, wo_id).await {
 wo_doc_map.insert(wo_id, wo.doc_number);
 }
 }

 // Compute aggregate stats
 let total_wage: rust_decimal::Decimal = summaries.iter().map(|s| s.total_amount).sum();
 let worker_count = summaries.len();
 let total_completed: rust_decimal::Decimal = summaries.iter()
 .flat_map(|s| s.details.iter())
 .map(|d| d.completed_qty)
 .sum();
 let total_defect: rust_decimal::Decimal = summaries.iter()
 .flat_map(|s| s.details.iter())
 .map(|d| d.defect_qty)
 .sum();
 let total_operator_defect: rust_decimal::Decimal = summaries.iter()
 .flat_map(|s| s.details.iter())
 .filter(|d| matches!(d.defect_reason, Some(abt_core::mes::enums::DefectReason::OperatorError)))
 .map(|d| d.defect_qty)
 .sum();

 let wctx = WageListContext {
 user_map: &user_map,
 wo_doc_map: &wo_doc_map,
 date_from,
 date_to,
 total_wage,
 worker_count,
 total_completed,
 total_defect,
 total_operator_defect,
 };
 let content = wage_list_page(&summaries, &wctx);
 Ok(Html(admin_page(is_htmx, "计件工资汇总", &claims, "production", WageListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

struct WageListContext<'a> {
 user_map: &'a HashMap<i64, String>,
 wo_doc_map: &'a HashMap<i64, String>,
 date_from: chrono::NaiveDate,
 date_to: chrono::NaiveDate,
 total_wage: rust_decimal::Decimal,
 worker_count: usize,
 total_completed: rust_decimal::Decimal,
 total_defect: rust_decimal::Decimal,
 total_operator_defect: rust_decimal::Decimal,
}

fn wage_list_page(
 summaries: &[abt_core::mes::work_report::WageSummary],
 ctx: &WageListContext,
) -> Markup {
 let date_from_str = ctx.date_from.format("%Y-%m-%d").to_string();
 let date_to_str = ctx.date_to.format("%Y-%m-%d").to_string();
 let total_completed_fmt = crate::utils::fmt_qty(ctx.total_completed);
 let total_defect_fmt = crate::utils::fmt_qty(ctx.total_defect);
 let defect_rate = if ctx.total_completed > rust_decimal::Decimal::ZERO {
 let rate = (ctx.total_defect / ctx.total_completed) * rust_decimal::Decimal::ONE_HUNDRED;
 format!("{:.1}%", rate)
 } else {
 "0%".to_string()
 };

 html! { div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "计件工资汇总" }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (maud::PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
 " 导出"
 }
 }
 }

 // 筛选栏
 div class="flex items-center gap-3 mb-5 flex-wrap" {
 div class="relative flex-1 max-w-xs" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>"#))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" placeholder="搜索工人姓名、工号…";
 }
 input type="date" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" value=(date_from_str) style="max-width:160px";
 span style="color:var(--muted);font-size:var(--text-sm);line-height:36px" { "至" }
 input type="date" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" value=(date_to_str) style="max-width:160px";
 }

 // 汇总统计卡片
 div class="grid gap-5" {
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:linear-gradient(135deg,#e6f4ff,#d6e8ff);color:var(--accent)" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1v22M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/></svg>"#))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums tabular-nums text-fg" { "¥" (crate::utils::fmt_qty(ctx.total_wage)) }
 div class="text-sm text-muted mt-1" { "本月工资总额" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:linear-gradient(135deg,#f0fff0,#e0ffe0);color:var(--success)" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"/></svg>"#))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums tabular-nums text-fg" { (ctx.worker_count) }
 div class="text-sm text-muted mt-1" { "计件工人数" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:linear-gradient(135deg,#fff8eb,#fff0d6);color:var(--warn)" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"/></svg>"#))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums tabular-nums text-fg" { (total_completed_fmt) }
 div class="text-sm text-muted mt-1" { "总完成数量" }
 div style="font-size:var(--text-xs);color:var(--muted);margin-top:2px" { "不良品 " (total_defect_fmt) " (" (defect_rate) ")" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
 div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:linear-gradient(135deg,#fff2f0,#ffe8e6);color:var(--danger)" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>"#))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums tabular-nums text-fg" { "—" }
 div class="text-sm text-muted mt-1" { "扣减金额(操作失误)" }
 div style="font-size:var(--text-xs);color:var(--muted);margin-top:2px" { "操作失误不良: " (crate::utils::fmt_qty(ctx.total_operator_defect)) "件" }
 }
 }
 }

 // 工资公式提示
 div class="flex items-start gap-2 p-3 bg-surface border border-border-soft rounded-sm text-sm text-fg-2" {
 (maud::PreEscaped(r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>"#))
 " 计算公式："
 code { "(完成数 + 非操作失误不良数) × 计件单价" }
 "，其中物料不良/设备故障/工艺问题照常计工资，操作失误不计工资"
 }

 // 工人工资明细卡片
 div class="wage-bg-white border border-border-soft rounded p-5" {
 div class="p-4 border-b" {
 div class="flex items-center gap-2 font-semibold text-base" {
 (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"/></svg>"#))
 " 工人工资明细"
 }
 }
 div class="wage-flex-1 overflow-y-auto" {
 // Header row
 div class="worker-row grid items-center gap-3 p-3 border-b-header" {
 span { "工人" }
 span { "完成数" }
 span { "不良品数" }
 span { "有效计件数" }
 span { "应发工资" }
 span {}
 }

 @if summaries.is_empty() {
 div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无工资数据" }
 }

 @for (idx, summary) in summaries.iter().enumerate() {
 @let worker_name = ctx.user_map.get(&summary.worker_id).cloned().unwrap_or_else(|| format!("工人#{}", summary.worker_id));
 @let initial = worker_name.chars().next().unwrap_or('?');
 @let wc = summary.details.iter().map(|d| d.completed_qty).sum::<rust_decimal::Decimal>();
 @let wd = summary.details.iter().map(|d| d.defect_qty).sum::<rust_decimal::Decimal>();
 @let we = summary.details.iter().map(|d| {
 let non_op = match d.defect_reason {
 Some(abt_core::mes::enums::DefectReason::OperatorError) => rust_decimal::Decimal::ZERO,
 _ => d.defect_qty,
 };
 d.completed_qty + non_op
 }).sum::<rust_decimal::Decimal>();
 @let toggle_id = format!("w{}", idx);

 // Worker summary row
 div class="grid items-center gap-3 p-3 border-b" style="cursor:pointer" _=(format!("on click if #{0}'s *display is 'none' then show #{0} else hide #{0}", toggle_id)) {
 div class="flex items-center gap-3" {
 div class="worker-inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none" style="background:var(--accent)" { (initial) }
 div class="flex flex-col" {
 span class="font-medium" { (worker_name) }
 }
 }
 span class="wage-font-mono tabular-nums" { (crate::utils::fmt_qty(wc)) }
 span class="wage-font-mono tabular-nums text-danger" { (crate::utils::fmt_qty(wd)) }
 span class="wage-font-mono tabular-nums" { (crate::utils::fmt_qty(we)) }
 span class="wage-font-mono tabular-nums text-success" style="font-weight:700" { "¥" (crate::utils::fmt_qty(summary.total_amount)) }
 span {
 (maud::PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 9l-7 7-7-7"/></svg>"#))
 }
 }

 // Expandable detail table
 div class="border-b bg-surface" id=(toggle_id) style="display:none" {
 table class="border-b bg-surface-table" {
 thead { tr {
 th { "工单" } th { "工序" } th { "完成" }
 th { "不良(原因)" } th { "有效数" } th { "单价" }
 th { "工资" }
 }}
 tbody {
 @for detail in &summary.details {
 @let wo_doc = ctx.wo_doc_map.get(&detail.work_order_id).cloned().unwrap_or_else(|| "—".to_string());
 @let defect_label = match detail.defect_reason {
 Some(abt_core::mes::enums::DefectReason::MaterialDefect) => format!("{} (物料不良)", crate::utils::fmt_qty(detail.defect_qty)),
 Some(abt_core::mes::enums::DefectReason::EquipmentFault) => format!("{} (设备故障)", crate::utils::fmt_qty(detail.defect_qty)),
 Some(abt_core::mes::enums::DefectReason::OperatorError) => format!("{} (操作失误)", crate::utils::fmt_qty(detail.defect_qty)),
 Some(abt_core::mes::enums::DefectReason::ProcessIssue) => format!("{} (工艺问题)", crate::utils::fmt_qty(detail.defect_qty)),
 None if detail.defect_qty > rust_decimal::Decimal::ZERO => crate::utils::fmt_qty(detail.defect_qty),
 _ => "—".to_string(),
 };
 @let non_op_defect = match detail.defect_reason {
 Some(abt_core::mes::enums::DefectReason::OperatorError) => rust_decimal::Decimal::ZERO,
 _ => detail.defect_qty,
 };
 @let effective = detail.completed_qty + non_op_defect;
 tr {
 td class="font-mono tabular-nums" { (wo_doc) }
 td { (detail.process_name) }
 td class="font-mono tabular-nums" { (crate::utils::fmt_qty(detail.completed_qty)) }
 td class="font-mono tabular-nums text-danger" { (defect_label) }
 td class="font-mono tabular-nums" { (crate::utils::fmt_qty(effective)) }
 td class="font-mono tabular-nums" { "¥" (detail.unit_price) }
 td class="font-mono tabular-nums text-success" { "¥" (crate::utils::fmt_qty(detail.wage_amount)) }
 }
 }
 }
 }
 }
 }
 }
 }

 // 分页
 div class="flex items-center justify-between py-4 px-5" {
 span { "共 " (ctx.worker_count) " 名工人" }
 }
 }}
}
