use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_batch::ProductionBatchService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::{BatchDetailPath, BatchListPath, BatchConfirmStepPath, BatchAdvancePath, BatchSuspendPath, BatchResumePath, BatchScrapPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

fn batch_status_label(s: &abt_core::mes::enums::BatchStatus) -> (&'static str, &'static str, &'static str) {
    use abt_core::mes::enums::BatchStatus::*;
    match s {
        Pending => ("待生产", "rgba(0,0,0,0.04)", "var(--muted)"),
        InProgress => ("进行中", "rgba(250,140,22,0.08)", "#fa8c16"),
        Suspended => ("已暂停", "rgba(245,63,63,0.06)", "#f53f3f"),
        PendingReceipt => ("待入库", "rgba(22,119,255,0.08)", "var(--accent)"),
        Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        Cancelled => ("已取消", "rgba(114,46,209,0.06)", "#722ed1"),
    }
}

#[require_permission("MES", "read")]
pub async fn get_batch_detail(path: BatchDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_batch_service();
    let batch = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let routings = svc.list_routings(&service_ctx, &mut conn, batch.work_order_id).await?;
    let product_name = svc.get_product_name(&mut conn, batch.product_id).await?.unwrap_or_default();
    let content = batch_detail_page(&batch, &product_name, &routings);
    Ok(Html(admin_page(is_htmx, "批次详情", &claims, "production", &format!("/admin/mes/batches/{}", path.id), "生产管理", Some(BatchListPath::PATH), content).into_string()))
}

#[require_permission("MES", "write")]
pub async fn confirm_step(path: BatchConfirmStepPath, ctx: RequestContext, axum::Form(form): axum::Form<ConfirmStepForm>) -> Result<impl IntoResponse> {
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
    svc.confirm_routing_step(&service_ctx, &mut conn, path.batch_id, form.step_no, req).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("MES", "write")]
pub async fn advance_to_receipt(path: BatchAdvancePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().advance_to_receipt(&service_ctx, &mut conn, path.batch_id).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("MES", "write")]
pub async fn suspend_batch(path: BatchSuspendPath, ctx: RequestContext, axum::Form(form): axum::Form<SuspendForm>) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().suspend(&service_ctx, &mut conn, path.batch_id, form.reason).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("MES", "write")]
pub async fn resume_batch(path: BatchResumePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().resume(&service_ctx, &mut conn, path.batch_id).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("MES", "write")]
pub async fn scrap_batch(path: BatchScrapPath, ctx: RequestContext, axum::Form(form): axum::Form<SuspendForm>) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().scrap(&service_ctx, &mut conn, path.batch_id, form.reason).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[derive(Debug, Deserialize)]
pub struct ConfirmStepForm {
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

#[derive(Debug, Deserialize)]
pub struct SuspendForm {
    pub reason: String,
}

fn batch_detail_page(batch: &abt_core::mes::production_batch::ProductionBatch, product_name: &str, routings: &[abt_core::mes::production_batch::WorkOrderRouting]) -> Markup {
    use abt_core::mes::enums::BatchStatus;
    let (sl, sb, sc) = batch_status_label(&batch.status);

    let current_step_display = if batch.current_step == 0 {
        html! { span style="color:var(--muted)" { "未开始" } }
    } else {
        let total = routings.len();
        let step_name = routings.iter()
            .find(|r| r.step_no == batch.current_step)
            .map(|r| r.process_name.as_str())
            .unwrap_or("—");
        html! { (batch.current_step) "/" (total) " " (step_name) }
    };

    html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(BatchListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "批次 " (batch.batch_no) } }
            div class="page-actions" {
                @if batch.status == BatchStatus::InProgress {
                    form hx-post=(format!("/admin/mes/batches/{}/suspend", batch.id)) hx-swap="none" style="display:inline" {
                        input type="hidden" name="reason" value="手动暂停";
                        button class="btn btn-default" type="submit" { "暂停" }
                    }
                }
                @if batch.status == BatchStatus::Suspended {
                    form hx-post=(format!("/admin/mes/batches/{}/resume", batch.id)) hx-swap="none" style="display:inline" {
                        button class="btn btn-primary" type="submit" { "恢复" }
                    }
                }
                @if batch.status == BatchStatus::PendingReceipt {
                    form hx-post=(format!("/admin/mes/batches/{}/advance", batch.id)) hx-swap="none" style="display:inline" {
                        button class="btn btn-primary" type="submit" { "推进入库" }
                    }
                }
            }
        }

        div class="info-card" {
            div class="info-grid" {
                div class="info-item" { label { "批次号" } span class="mono" { (batch.batch_no) } }
                div class="info-item" { label { "流转卡号" } span class="mono" { (batch.card_sn) } }
                div class="info-item" { label { "产品" } span { (product_name) } }
                div class="info-item" { label { "数量" } span class="mono" { (crate::utils::fmt_qty(batch.batch_qty)) } }
                div class="info-item" { label { "已完成" } span class="mono" { (crate::utils::fmt_qty(batch.completed_qty)) } }
                div class="info-item" { label { "报废" } span class="mono" { (crate::utils::fmt_qty(batch.scrap_qty)) } }
                div class="info-item" { label { "当前工序" } span { (current_step_display) } }
                div class="info-item" { label { "状态" } span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", sb, sc)) { (sl) } }
            }
        }

        // Routing steps table
        @if !routings.is_empty() {
            div class="form-section" style="margin-top:var(--space-6)" {
                div class="form-section-title" { "工序路线" }
                div class="data-card" {
                    div class="data-card-scroll" {
                        table class="data-table" { thead { tr {
                            th { "序号" } th { "工序名称" } th class="num-right" { "计划数" }
                            th class="num-right" { "完成数" } th class="num-right" { "不良数" } th { "状态" }
                        }} tbody {
                            @for r in routings {
                                @let is_current = r.step_no == batch.current_step;
                                tr style=(if is_current { "background:rgba(22,119,255,0.04);font-weight:600" } else { "" }) {
                                    td class="mono" { (r.step_no) }
                                    td { (r.process_name) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(r.planned_qty)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(r.completed_qty)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(r.defect_qty)) }
                                    td { (fmt_routing_status(&r.status)) }
                                }
                            }
                        }}
                    }
                }
            }
        }

        // Report form for current step
        @if batch.status == BatchStatus::Pending || batch.status == BatchStatus::InProgress {
            div class="form-section" style="margin-top:var(--space-6)" {
                div class="form-section-title" { "报工" }
                form hx-post=(format!("/admin/mes/batches/{}/confirm-step", batch.id)) hx-swap="none" {
                    div class="form-grid" {
                        div class="form-field" { label class="form-label" { "工序号" } input class="form-input" type="number" name="step_no" value=(batch.current_step + 1) style="width:80px"; }
                        div class="form-field" { label class="form-label" { "工人ID" } input class="form-input" type="number" name="worker_id" required; }
                        div class="form-field" { label class="form-label" { "班次" } select class="form-select" name="shift" {
                            option value="1" { "白班" } option value="2" { "夜班" }
                        }}
                        div class="form-field" { label class="form-label" { "完成数量" } input class="form-input" type="number" step="0.01" name="completed_qty" required; }
                        div class="form-field" { label class="form-label" { "不良数量" } input class="form-input" type="number" step="0.01" name="defect_qty" value="0"; }
                        div class="form-field" { label class="form-label" { "工时" } input class="form-input" type="number" step="0.01" name="work_hours" required; }
                        div class="form-field" { label class="form-label" { "报工日期" } input class="form-input" type="date" name="report_date" required; }
                    }
                    div style="margin-top:var(--space-4)" { button type="submit" class="btn btn-primary" { "提交报工" } }
                }
            }
        }
    }}
}

fn fmt_routing_status(s: &abt_core::mes::enums::RoutingStatus) -> Markup {
    use abt_core::mes::enums::RoutingStatus;
    let (label, bg, color) = match s {
        RoutingStatus::Pending => ("待开始", "rgba(0,0,0,0.04)", "var(--muted)"),
        RoutingStatus::InProgress => ("进行中", "rgba(250,140,22,0.08)", "#fa8c16"),
        RoutingStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        RoutingStatus::Skipped => ("已跳过", "rgba(114,46,209,0.06)", "#722ed1"),
    };
    html! { span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{bg};color:{color}")) { (label) } }
}
