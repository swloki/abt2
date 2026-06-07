use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::work_report::WorkReportService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::{ReportDetailPath, ReportListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_report_detail(path: ReportDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.work_report_service();
    let report = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let shift_label = match report.shift { abt_core::mes::enums::ShiftType::Day => "白班", _ => "夜班" };
    let defect_label = report.defect_reason.map(|d| match d {
        abt_core::mes::enums::DefectReason::MaterialDefect => "物料不良",
        abt_core::mes::enums::DefectReason::EquipmentFault => "设备故障",
        abt_core::mes::enums::DefectReason::OperatorError => "操作失误",
        abt_core::mes::enums::DefectReason::ProcessIssue => "工艺问题",
    }).unwrap_or("\u{2014}");

    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(ReportListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "报工 " (report.doc_number) } }
        }
        div class="info-card" {
            div class="info-grid" {
                div class="info-item" { label { "单号" } span class="mono" { (report.doc_number) } }
                div class="info-item" { label { "工单ID" } span { (report.work_order_id) } }
                div class="info-item" { label { "批次ID" } span { (report.batch_id) } }
                div class="info-item" { label { "工序ID" } span { (report.routing_id) } }
                div class="info-item" { label { "报工日期" } span { (report.report_date) } }
                div class="info-item" { label { "班次" } span { (shift_label) } }
                div class="info-item" { label { "工人ID" } span { (report.worker_id) } }
                div class="info-item" { label { "完成数量" } span class="mono" { (crate::utils::fmt_qty(report.completed_qty)) } }
                div class="info-item" { label { "不良数量" } span class="mono" { (crate::utils::fmt_qty(report.defect_qty)) } }
                div class="info-item" { label { "不良原因" } span { (defect_label) } }
                div class="info-item" { label { "工时" } span class="mono" { (crate::utils::fmt_qty(report.work_hours)) } }
                div class="info-item" { label { "创建时间" } span { (report.created_at.format("%Y-%m-%d %H:%M")) } }
                @if !report.remark.is_empty() {
                    div class="info-item span-2" { label { "备注" } span { (report.remark) } }
                }
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "报工详情", &claims, "production", &format!("/admin/mes/reports/{}", path.id), "生产管理", Some(ReportListPath::PATH), content).into_string()))
}
