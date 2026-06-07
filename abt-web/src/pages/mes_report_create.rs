use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_batch::ProductionBatchService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::{ReportCreatePath, ReportListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct ReportCreateForm {
    pub batch_id: i64,
    pub step_no: i32,
    pub worker_id: i64,
    pub shift: abt_core::mes::enums::ShiftType,
    pub completed_qty: rust_decimal::Decimal,
    pub defect_qty: rust_decimal::Decimal,
    pub defect_reason: Option<abt_core::mes::enums::DefectReason>,
    pub work_hours: rust_decimal::Decimal,
    pub report_date: chrono::NaiveDate,
    pub remark: Option<String>,
}

#[require_permission("MES", "write")]
pub async fn get_report_create(_path: ReportCreatePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = report_create_page();
    Ok(Html(admin_page(is_htmx, "新建报工", &claims, "production", ReportCreatePath::PATH, "生产管理", Some(ReportListPath::PATH), content).into_string()))
}

#[require_permission("MES", "write")]
pub async fn create_report(
    _path: ReportCreatePath, ctx: RequestContext, axum::Form(form): axum::Form<ReportCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let req = abt_core::mes::production_batch::StepConfirmationReq {
        step_no: form.step_no,
        worker_id: form.worker_id,
        shift: form.shift,
        completed_qty: form.completed_qty,
        defect_qty: form.defect_qty,
        defect_reason: form.defect_reason,
        work_hours: form.work_hours,
        report_date: form.report_date,
        remark: form.remark,
    };
    svc.confirm_routing_step(&service_ctx, &mut conn, form.batch_id, form.step_no, req).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", ReportListPath::PATH).body(axum::body::Body::empty()).unwrap())
}

fn report_create_page() -> Markup {
    html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(ReportListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "新建报工" } }
        }
        form hx-post=(ReportCreatePath::PATH) hx-swap="none" {
            div class="form-section" {
                div class="form-section-title" { "报工信息" }
                div class="form-grid" {
                    div class="form-field" { label class="form-label" { "批次ID" } input class="form-input" type="number" name="batch_id" required; }
                    div class="form-field" { label class="form-label" { "工序号" } input class="form-input" type="number" name="step_no" required; }
                    div class="form-field" { label class="form-label" { "工人ID" } input class="form-input" type="number" name="worker_id" required; }
                    div class="form-field" { label class="form-label" { "班次" } select class="form-select" name="shift" {
                        option value="1" { "白班" } option value="2" { "夜班" }
                    }}
                    div class="form-field" { label class="form-label" { "完成数量" } input class="form-input" type="number" step="0.01" name="completed_qty" required; }
                    div class="form-field" { label class="form-label" { "不良数量" } input class="form-input" type="number" step="0.01" name="defect_qty" value="0"; }
                    div class="form-field" { label class="form-label" { "不良原因" } select class="form-select" name="defect_reason" {
                        option value="" { "无" }
                        option value="1" { "物料不良" } option value="2" { "设备故障" }
                        option value="3" { "操作失误" } option value="4" { "工艺问题" }
                    }}
                    div class="form-field" { label class="form-label" { "工时" } input class="form-input" type="number" step="0.01" name="work_hours" required; }
                    div class="form-field" { label class="form-label" { "报工日期" } input class="form-input" type="date" name="report_date" required; }
                }
            }
            div class="create-action-bar" {
                a class="btn btn-default" href=(ReportListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "提交" }
            }
        }
    }}
}
