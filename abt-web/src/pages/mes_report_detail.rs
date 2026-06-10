use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

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
        div class="batch-detail-header" {
            div class="batch-detail-title-row" {
                div class="detail-doc-no" { (report.doc_number) " " span class="status-pill status-completed" { "已确认" } }
            }
            div class="detail-info-grid-5" {
                div class="detail-info-item" { span class="detail-info-label" { "工单" } span class="detail-info-value" { (wo) } }
                div class="detail-info-item" { span class="detail-info-label" { "批次" } span class="detail-info-value" { (batch) } }
                div class="detail-info-item" { span class="detail-info-label" { "工序" } span class="detail-info-value" { (process) } }
                div class="detail-info-item" { span class="detail-info-label" { "班次" } span class="detail-info-value" { (shift_label) } }
                div class="detail-info-item" { span class="detail-info-label" { "工人" } span class="detail-info-value" { (worker) } }
                div class="detail-info-item" { span class="detail-info-label" { "完成数量" } span class="detail-info-value text-success" { (crate::utils::fmt_qty(report.completed_qty)) } }
                div class="detail-info-item" { span class="detail-info-label" { "不良数量" } span class="detail-info-value text-danger" { (crate::utils::fmt_qty(report.defect_qty)) } }
                div class="detail-info-item" { span class="detail-info-label" { "不良原因" } span class="detail-info-value" { (defect_label) } }
                div class="detail-info-item" { span class="detail-info-label" { "实际工时" } span class="detail-info-value" { (crate::utils::fmt_qty(report.work_hours)) " h" } }
                div class="detail-info-item" { span class="detail-info-label" { "报工日期" } span class="detail-info-value" { (report.report_date) } }
                div class="detail-info-item" { span class="detail-info-label" { "创建人" } span class="detail-info-value" { (worker) } }
                div class="detail-info-item" { span class="detail-info-label" { "创建时间" } span class="detail-info-value" { (report.created_at.format("%Y-%m-%d %H:%M")) } }
                div class="detail-info-item" { span class="detail-info-label" { "备注" } span class="detail-info-value" { (if report.remark.is_empty() { "—".to_string() } else { report.remark.clone() }) } }
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "报工详情", &claims, "production", &format!("/admin/mes/reports/{}", path.id), "生产管理", Some(ReportListPath::PATH), content, &nav_filter).into_string()))
}
