use axum::response::{Html, IntoResponse};
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::production_batch::repo::BatchRoutingProgressRepo;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::WorkReportService;
use abt_core::shared::identity::UserService;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::{BatchDetailPath, BatchConfirmStepPath, BatchAdvancePath, BatchSuspendPath, BatchResumePath, BatchScrapPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

fn batch_status_label(s: &abt_core::mes::enums::BatchStatus) -> (&'static str, &'static str) {
    use abt_core::mes::enums::BatchStatus::*;
    match s {
        Pending => ("待生产", "status-draft"),
        InProgress => ("进行中", "status-progress"),
        Suspended => ("已暂停", "status-suspended"),
        PendingReceipt => ("待入库", "status-inspecting"),
        Completed => ("已完成", "status-completed"),
        Cancelled => ("已取消", "status-neutral"),
    }
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_detail(path: BatchDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let wr_svc = state.work_report_service();
    let user_svc = state.user_service();
    let batch = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let routings = svc.list_routings(&service_ctx, &mut conn, batch.work_order_id).await?;
    let product_name = svc.get_product_name(&mut conn, batch.product_id).await?.unwrap_or_default();
    let wo = wo_svc.find_by_id(&service_ctx, &mut conn, batch.work_order_id).await?;

    // 报工记录
    let reports = wr_svc.list_by_batch(&service_ctx, &mut conn, batch.id).await.unwrap_or_default();

    // 批量获取工人名和创建人名
    let mut user_ids: Vec<i64> = reports.iter().map(|r| r.worker_id).collect();
    user_ids.push(batch.operator_id);
    user_ids.sort_unstable();
    user_ids.dedup();
    let users = user_svc.get_users_by_ids(&service_ctx, &mut conn, user_ids).await.unwrap_or_default();
    let user_map: std::collections::HashMap<i64, String> = users.iter()
        .map(|u| (u.user.user_id, u.user.display_name.clone().unwrap_or_else(|| u.user.username.clone())))
        .collect();
    let creator_name = user_map.get(&batch.operator_id).cloned().unwrap_or_else(|| "—".to_string());

    // 工序名映射
    let routing_map: std::collections::HashMap<i64, &str> = routings.iter()
        .map(|r| (r.id, r.process_name.as_str()))
        .collect();
    // 查询批次工序执行进度（写真相源），用于工序流转进度展示
    let progress_list = BatchRoutingProgressRepo::list_by_batch(&mut *conn, batch.id).await?;
    let progress_map: std::collections::HashMap<i64, &abt_core::mes::production_batch::BatchRoutingProgress> =
        progress_list.iter().map(|p| (p.routing_id, p)).collect();

    let content = batch_detail_page(&batch, &product_name, &wo, &routings, &reports, &routing_map, &user_map, &creator_name, &progress_map);
    Ok(Html(admin_page(is_htmx, "批次详情", &claims, "production", &format!("/admin/mes/batches/{}", path.id), "生产管理", Some(&format!("/admin/mes/orders/{}", wo.id)), content, &nav_filter).into_string()))
}

#[require_permission("WORK_ORDER", "update")]
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

#[require_permission("WORK_ORDER", "update")]
pub async fn advance_to_receipt(path: BatchAdvancePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().advance_to_receipt(&service_ctx, &mut conn, path.batch_id).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("WORK_ORDER", "update")]
pub async fn suspend_batch(path: BatchSuspendPath, ctx: RequestContext, axum::Form(form): axum::Form<SuspendForm>) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().suspend(&service_ctx, &mut conn, path.batch_id, form.reason).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("WORK_ORDER", "update")]
pub async fn resume_batch(path: BatchResumePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_batch_service().resume(&service_ctx, &mut conn, path.batch_id).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/batches/{}", path.batch_id)).body(axum::body::Body::empty()).unwrap())
}

#[require_permission("WORK_ORDER", "update")]
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

fn batch_detail_page(
    batch: &abt_core::mes::production_batch::ProductionBatch,
    product_name: &str,
    wo: &abt_core::mes::work_order::WorkOrder,
    routings: &[abt_core::mes::production_batch::WorkOrderRouting],
    reports: &[abt_core::mes::work_report::WorkReport],
    routing_map: &std::collections::HashMap<i64, &str>,
    user_map: &std::collections::HashMap<i64, String>,
    creator_name: &str,
    progress_map: &std::collections::HashMap<i64, &abt_core::mes::production_batch::BatchRoutingProgress>,
) -> Markup {
    use abt_core::mes::enums::BatchStatus;
    let (sl, sc) = batch_status_label(&batch.status);

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

    let shift_label = |s: &abt_core::mes::enums::ShiftType| -> &'static str {
        use abt_core::mes::enums::ShiftType;
        match s { ShiftType::Day => "白班", ShiftType::Night => "夜班" }
    };

    let defect_label = |d: &Option<abt_core::mes::enums::DefectReason>| -> &'static str {
        use abt_core::mes::enums::DefectReason;
        match d {
            None => "—",
            Some(DefectReason::MaterialDefect) => "物料不良",
            Some(DefectReason::EquipmentFault) => "设备故障",
            Some(DefectReason::OperatorError) => "操作失误",
            Some(DefectReason::ProcessIssue) => "工艺问题",
        }
    };

    html! { div {
        // 工单上下文条
        a class="back-link" href=(format!("/admin/mes/orders/{}", wo.id)) {
            (crate::components::icon::chevron_left_icon("w-4 h-4"))
            "返回工单 " span class="mono" { (wo.doc_number.as_str()) }
        }
        div class="batch-detail-header" {
            div class="batch-detail-title-row" {
                div class="detail-doc-no" {
                    (batch.batch_no)
                    span class=(format!("status-pill {sc}")) { (sl) }
                    span class="time-cell" style="font-weight:400;margin-left:var(--space-2)" { "流转卡: " (batch.card_sn) }
                }
                div style="display:flex;gap:var(--space-3)" {
                    @if batch.status == BatchStatus::InProgress {
                        a class="btn btn-default" href=(format!("/admin/mes/batches/{}/suspend", batch.id)) { "暂停" }
                        a class="btn btn-primary" href=(format!("/admin/mes/reports/create?batch_id={}", batch.id)) { "工序报工" }
                    }
                    @if batch.status == BatchStatus::Suspended {
                        form hx-post=(format!("/admin/mes/batches/{}/resume", batch.id)) hx-swap="none" style="display:inline" {
                            button class="btn btn-primary" type="submit" { "恢复" }
                        }
                    }
                    @if batch.status == BatchStatus::PendingReceipt {
                        a class="btn btn-primary" href=(format!("/admin/mes/receipts/create?batch_id={}", batch.id)) { "入库" }
                    }
                }
            }
            // 10 fields matching prototype order
            div class="detail-info-grid-5" {
                div class="detail-info-item" { span class="detail-info-label" { "工单" } span class="detail-info-value" { a href=(format!("/admin/mes/orders/{}", wo.id)) class="link-cell" { (wo.doc_number) } } }
                @if let (Some(pid), Some(pdoc)) = (wo.source_plan_id, wo.source_plan_doc.as_ref()) {
                    div class="detail-info-item" { span class="detail-info-label" { "计划" } span class="detail-info-value" { a href=(format!("/admin/mes/plans/{}", pid)) class="link-cell" { (pdoc) } } }
                }
                div class="detail-info-item" { span class="detail-info-label" { "产品" } span class="detail-info-value" { (product_name) } }
                div class="detail-info-item" { span class="detail-info-label" { "班组" } span class="detail-info-value" { "—" } }
                div class="detail-info-item" { span class="detail-info-label" { "批次数量" } span class="detail-info-value" { (crate::utils::fmt_qty(batch.batch_qty)) } }
                div class="detail-info-item" { span class="detail-info-label" { "完成/报废" } span class="detail-info-value" { span class="text-success" { (crate::utils::fmt_qty(batch.completed_qty)) } " / " span class="text-danger" { (crate::utils::fmt_qty(batch.scrap_qty)) } } }
                div class="detail-info-item" { span class="detail-info-label" { "当前工序" } span class="detail-info-value" style="color:var(--warn)" { (current_step_display) } }
                div class="detail-info-item" { span class="detail-info-label" { "实际开始" } span class="detail-info-value" { (batch.actual_start.map(|t| t.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".to_string())) } }
                div class="detail-info-item" { span class="detail-info-label" { "实际结束" } span class="detail-info-value text-muted" { (batch.actual_end.map(|t| t.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".to_string())) } }
                div class="detail-info-item" { span class="detail-info-label" { "创建人" } span class="detail-info-value" { (creator_name) } }
                div class="detail-info-item" { span class="detail-info-label" { "创建时间" } span class="detail-info-value" { (batch.created_at.format("%Y-%m-%d %H:%M").to_string()) } }
            }
        }

        // ── 工序流转进度 (horizontal step dots) ──
        @if !routings.is_empty() {
            div class="sub-section" {
                div class="sub-section-title" { "工序流转进度" }
                div class="progress-track" {
                    @for (i, r) in routings.iter().enumerate() {
                        @let brp = progress_map.get(&r.id);
                        @let is_completed = brp.map(|b| b.status == abt_core::mes::enums::RoutingStatus::Completed).unwrap_or(false);
                        @let is_active = r.step_no == batch.current_step;
                        @let cls = if is_completed { "progress-step completed" } else if is_active { "progress-step active" } else { "progress-step" };
                        div class=(cls) {
                            div class="progress-step-dot" {
                                @if is_completed { "\u{2713}" } @else { (r.step_no) }
                            }
                            div class="progress-step-label" { (r.process_name) }
                            // 每步完成量/不良量
                            @if let Some(p) = brp {
                                @if p.completed_qty > rust_decimal::Decimal::ZERO || p.defect_qty > rust_decimal::Decimal::ZERO {
                                    div style="font-size:var(--text-xs);color:var(--text-muted);margin-top:2px;white-space:nowrap" {
                                        span style="color:var(--success)" { (crate::utils::fmt_qty(p.completed_qty)) }
                                        @if p.defect_qty > rust_decimal::Decimal::ZERO {
                                            " / " span style="color:var(--danger)" { (crate::utils::fmt_qty(p.defect_qty)) }
                                        }
                                    }
                                }
                            }
                            @if i < routings.len() - 1 {
                                div class="progress-step-line" {}
                            }
                        }
                    }
                }
            }
        }

        // ── 报工记录 (matches prototype sub-section) ──
        div class="sub-section" {
            div class="sub-section-title" { "报工记录" }
            @if reports.is_empty() {
                div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无报工记录" }
            } @else {
                div class="data-card-scroll" {
                    table class="data-table" style="width:100%" {
                        thead {
                            tr {
                                th { "报工单号" }
                                th { "工序" }
                                th { "班次" }
                                th { "工人" }
                                th { "完成数量" }
                                th { "不良数量" }
                                th { "不良原因" }
                                th { "工时(h)" }
                                th { "报工时间" }
                            }
                        }
                        tbody {
                            @for r in reports {
                                tr {
                                    td { a href=(format!("/admin/mes/reports/{}", r.id)) class="link-cell mono" { (r.doc_number) } }
                                    td { (routing_map.get(&r.routing_id).copied().unwrap_or("—")) }
                                    td { span class="status-pill status-neutral" { (shift_label(&r.shift)) } }
                                    td { (user_map.get(&r.worker_id).map(|s| s.as_str()).unwrap_or("—")) }
                                    td class="mono text-success" { (crate::utils::fmt_qty(r.completed_qty)) }
                                    td class="mono text-danger" { (crate::utils::fmt_qty(r.defect_qty)) }
                                    td { (defect_label(&r.defect_reason)) }
                                    td class="mono" { (crate::utils::fmt_qty(r.work_hours)) }
                                    td class="time-cell" { (r.created_at.format("%Y-%m-%d %H:%M").to_string()) }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── 状态变更记录 (matches prototype sub-section) ──
        div class="sub-section" {
            div class="sub-section-title" { "状态变更记录" }
            table class="data-table" style="width:100%" {
                thead {
                    tr {
                        th { "时间" }
                        th { "操作" }
                        th { "变更" }
                        th { "操作人" }
                        th { "备注" }
                    }
                }
                tbody {
                    // 创建记录
                    tr {
                        td class="time-cell" { (batch.created_at.format("%Y-%m-%d %H:%M").to_string()) }
                        td { span class="status-pill status-draft" { "创建" } }
                        td { "批次创建" }
                        td { (creator_name) }
                        td class="text-muted" { "工单下达自动生成" }
                    }
                    // 实际开始记录
                    @if let Some(start) = batch.actual_start {
                        tr {
                            td class="time-cell" { (start.format("%Y-%m-%d %H:%M").to_string()) }
                            td { span class="status-pill status-confirmed" { "开始生产" } }
                            td { "待生产 → 进行中" }
                            td { (creator_name) }
                            td class="text-muted" { "首道工序开始" }
                        }
                    }
                }
            }
        }
    }}
}

