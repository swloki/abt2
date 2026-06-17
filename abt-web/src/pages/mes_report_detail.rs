use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use abt_core::mes::work_report::WorkReportService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::{ReportDetailPath, ReportListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("WORK_ORDER", "read")]
pub async fn get_report_detail(path: ReportDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.work_report_service();
 let report = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let lookups = svc.get_detail_lookups(&mut conn, &report).await?;

 let shift_label = match report.shift { abt_core::mes::enums::ShiftType::Day => "白班", _ => "夜班" };
 let defect_label = report.defect_reason.map(|d| match d {
 abt_core::mes::enums::DefectReason::MaterialDefect => "物料不良",
 abt_core::mes::enums::DefectReason::EquipmentFault => "设备故障",
 abt_core::mes::enums::DefectReason::OperatorError => "操作失误",
 abt_core::mes::enums::DefectReason::ProcessIssue => "工艺问题",
 }).unwrap_or("\u{2014}");

 let wo = lookups.wo_doc_number.as_deref().unwrap_or("—");
 let batch = lookups.batch_no.as_deref().unwrap_or("—");
 let process = lookups.process_name.as_deref().unwrap_or("—");
 let worker = lookups.worker_name.as_deref().unwrap_or("—");

 let content = html! { div {
 div class="block bg-bg border border-border rounded p-6" {
 div class="flex items-center justify-between" {
 div class="text-[24px] font-bold text-fg flex items-center gap-[14px]" { (report.doc_number) " " span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#f0fff0] text-[#389e0d]" { "已确认" } }
 }
 div class="grid gap-5 gap-4-5" {
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "工单" } span class="detail-text-sm text-fg font-medium" { (wo) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "批次" } span class="detail-text-sm text-fg font-medium" { (batch) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "工序" } span class="detail-text-sm text-fg font-medium" { (process) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "班次" } span class="detail-text-sm text-fg font-medium" { (shift_label) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "工人" } span class="detail-text-sm text-fg font-medium" { (worker) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "完成数量" } span class="detail-text-sm text-fg font-medium text-success" { (crate::utils::fmt_qty(report.completed_qty)) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "不良数量" } span class="detail-text-sm text-fg font-medium text-danger" { (crate::utils::fmt_qty(report.defect_qty)) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "不良原因" } span class="detail-text-sm text-fg font-medium" { (defect_label) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "实际工时" } span class="detail-text-sm text-fg font-medium" { (crate::utils::fmt_qty(report.work_hours)) " h" } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "报工日期" } span class="detail-text-sm text-fg font-medium" { (report.report_date) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "创建人" } span class="detail-text-sm text-fg font-medium" { (worker) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "创建时间" } span class="detail-text-sm text-fg font-medium" { (report.created_at.format("%Y-%m-%d %H:%M")) } }
 div class="detail-flex flex-col gap-1" { span class="detail-text-xs text-muted font-medium" { "备注" } span class="detail-text-sm text-fg font-medium" { (if report.remark.is_empty() { "—".to_string() } else { report.remark.clone() }) } }
 }
 }

 // 工资计算明细
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "工资计算" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 label { "完成数量" }
 span class="font-mono tabular-nums" { (crate::utils::fmt_qty(report.completed_qty)) }
 }
 div class="flex flex-col gap-1" {
 label { "不良数量" }
 span class="font-mono tabular-nums" { (crate::utils::fmt_qty(report.defect_qty)) }
 }
 div class="flex flex-col gap-1" {
 label { "合格数量" }
 span class="font-mono tabular-nums" { (crate::utils::fmt_qty(report.completed_qty - report.defect_qty)) }
 }
 }
 div class="calc-detail" {
 div class="flex gap-[12px]" {
 span class="w-[80px] text-[#999] text-[13px]" { "工序" }
 span class="text-[#333] text-[13px]" { (process) }
 }
 div class="flex gap-[12px]" {
 span class="w-[80px] text-[#999] text-[13px]" { "实际工时" }
 span class="text-[#333] text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(report.work_hours)) " h" }
 }
 div class="bg-white text-[13px]" {
 "合格量 = 完成量(" (crate::utils::fmt_qty(report.completed_qty)) ")"
 " - 不良量(" (crate::utils::fmt_qty(report.defect_qty)) ")"
 " = "
 strong { (crate::utils::fmt_qty(report.completed_qty - report.defect_qty)) " 件" }
 }
 }
 }
 }};
 Ok(Html(admin_page(is_htmx, "报工详情", &claims, "production", &format!("/admin/mes/reports/{}", path.id), "生产管理", Some(ReportListPath::PATH), content, &nav_filter).into_string()))
}
