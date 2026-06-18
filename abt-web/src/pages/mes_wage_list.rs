use chrono::Datelike;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::WorkReportService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
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
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "计件工资汇总" }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::download_icon("w-4 h-4"))
 "导出"
 }
 }
 }

 // ── 筛选栏 ──
 div class="flex items-center gap-3 mb-5 flex-wrap" {
 div class="relative w-60" {
 (icon::search_icon("absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" placeholder="搜索工人姓名、工号…";
 }
 input type="date" class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" value=(date_from_str);
 span class="text-sm text-muted" { "至" }
 input type="date" class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" value=(date_to_str);
 }

 // ── 汇总统计卡片 ──
 div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-5" {
 // 工资总额
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[#e6f4ff] text-accent" {
 (icon::dollar_icon("w-5 h-5"))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { "¥" (crate::utils::fmt_qty(ctx.total_wage)) }
 div class="text-sm text-muted mt-1" { "本月工资总额" }
 }
 }
 // 计件工人数
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[#f0fff0] text-success" {
 (icon::users_icon("w-5 h-5"))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { (ctx.worker_count) }
 div class="text-sm text-muted mt-1" { "计件工人数" }
 }
 }
 // 完成数量
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[#fff8eb] text-warn" {
 (icon::package_icon("w-5 h-5"))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { (total_completed_fmt) }
 div class="text-sm text-muted mt-1" { "总完成数量" }
 div class="text-xs text-muted mt-0.5" { "不良品 " (total_defect_fmt) " (" (defect_rate) ")" }
 }
 }
 // 扣减金额
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[#fff2f0] text-danger" {
 (icon::alert_triangle_icon("w-5 h-5"))
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { "—" }
 div class="text-sm text-muted mt-1" { "扣减金额(操作失误)" }
 div class="text-xs text-muted mt-0.5" { "操作失误不良: " (crate::utils::fmt_qty(ctx.total_operator_defect)) "件" }
 }
 }
 }

 // ── 工资公式提示 ──
 div class="flex items-start gap-2 p-3 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 mb-5" {
 (icon::info_icon("w-3.5 h-3.5 mt-0.5 shrink-0"))
 span {
 "计算公式："
 code class="px-1 py-0.5 bg-bg rounded text-xs" { "(完成数 + 非操作失误不良数) × 计件单价" }
 "，其中物料不良/设备故障/工艺问题照常计工资，操作失误不计工资"
 }
 }

 // ── 工人工资明细 ──
 div class="data-card" {
 div class="flex items-center gap-2 font-semibold text-fg px-5 py-4 border-b border-border-soft" {
 (icon::users_icon("w-5 h-5"))
 "工人工资明细"
 }

 // Header row
 div class="grid grid-cols-[1fr_80px_80px_80px_100px_40px] items-center gap-3 px-5 py-2 text-xs text-muted font-semibold uppercase tracking-wide border-b border-border-soft" {
 span { "工人" }
 span class="text-right" { "完成数" }
 span class="text-right" { "不良品" }
 span class="text-right" { "有效数" }
 span class="text-right" { "应发工资" }
 span {}
 }

 @if summaries.is_empty() {
 div class="text-center text-muted text-sm py-8" { "暂无工资数据" }
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
 div class="grid grid-cols-[1fr_80px_80px_80px_100px_40px] items-center gap-3 px-5 py-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors duration-100"
 _=(format!("on click toggle .expanded on #{0} then toggle .rotate-180 on #{0}-icon", toggle_id)) {
 div class="flex items-center gap-3" {
 div class="w-8 h-8 rounded-full grid place-items-center text-white font-semibold shrink-0 bg-accent" { (initial) }
 span class="font-medium text-fg" { (worker_name) }
 }
 span class="text-right font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(wc)) }
 span class="text-right font-mono tabular-nums text-danger" { (crate::utils::fmt_qty(wd)) }
 span class="text-right font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(we)) }
 span class="text-right font-mono tabular-nums text-success font-bold" { "¥" (crate::utils::fmt_qty(summary.total_amount)) }
 span class="text-muted" {
 svg id=(format!("{}-icon", toggle_id)) viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4 transition-transform duration-200 rotate-180" {
 polyline points="6 9 12 15 18 9" {}
 }
 }
 }

 // Expandable detail table — animated grid-rows expansion
 div class="bg-surface grid transition-all duration-200 ease-in-out grid-rows-[0fr] expanded:grid-rows-[1fr] overflow-hidden" id=(toggle_id) {
 div class="overflow-hidden" {
 table class="w-full data-table" {
 thead { tr {
 th { "工单" } th { "工序" } th class="text-right" { "完成" }
 th class="text-right" { "不良(原因)" } th class="text-right" { "有效数" } th class="text-right" { "单价" }
 th class="text-right" { "工资" }
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
 td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(detail.completed_qty)) }
 td class="text-right font-mono tabular-nums text-danger" { (defect_label) }
 td class="text-right font-mono tabular-nums" { (crate::utils::fmt_qty(effective)) }
 td class="text-right font-mono tabular-nums" { "¥" (detail.unit_price) }
 td class="text-right font-mono tabular-nums text-success" { "¥" (crate::utils::fmt_qty(detail.wage_amount)) }
 }
 }
 }
 }
 }
 }
 }

 // 分页
 div class="flex items-center justify-between px-5 py-4" {
 span class="text-sm text-muted" { "共 " (ctx.worker_count) " 名工人" }
 }
 }
 }}
}
