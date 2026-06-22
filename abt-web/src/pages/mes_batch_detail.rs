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
 html! {
    span class="text-muted" { "未开始" }
}
 } else {
 let total = routings.len();
 let step_name = routings.iter()
 .find(|r| r.step_no == batch.current_step)
 .map(|r| r.process_name.as_str())
 .unwrap_or("—");
 html! {
    (batch.current_step)
    "/"
    (total)
    " "
    (step_name)
}
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

 html! {
    div class="space-y-5" {
        // 工单上下文条
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("/admin/mes/orders/{}", wo.id))
        {
            (crate::components::icon::chevron_left_icon("w-4 h-4"))
            "返回工单 "
            span class="font-mono tabular-nums" { (wo.doc_number.as_str()) }
        }
        div class="bg-bg border border-border-soft rounded-xl p-5" {
            div class="flex items-center justify-between mb-4" {
                div class="text-xl font-bold text-fg flex items-center gap-[14px]" {
                    (batch.batch_no)
                    span class=(format!("status-pill {}", crate::utils::status_color(sc))) { (sl) }
                    span class="text-muted text-[13px] font-normal ml-2" { "流转卡: " (batch.card_sn) }
                }
                div class="flex gap-3" {
                    @if batch.status == BatchStatus::InProgress {
                        a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            href=(format!("/admin/mes/batches/{}/suspend", batch.id))
                        { "暂停" }
                        a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                            href=(format!("/admin/mes/reports/create?batch_id={}", batch.id))
                        { "工序报工" }
                    }
                    @if batch.status == BatchStatus::Suspended {
                        form
                            hx-post=(format!("/admin/mes/batches/{}/resume", batch.id))
                            hx-swap="none"
                            class="inline"
                        {
                            button
                                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                                type="submit"
                            { "恢复" }
                        }
                    }
                    @if batch.status == BatchStatus::PendingReceipt {
                        a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                            href=(format!("/admin/mes/receipts/create?batch_id={}", batch.id))
                        { "入库" }
                    }
                }
            }
            // 信息字段 — 多列紧凑布局
            div class="grid grid-cols-2 md:grid-cols-4 gap-x-6 gap-y-3" {
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "工单" }
                    span class="text-sm text-fg font-medium" {
                        a   href=(format!("/admin/mes/orders/{}", wo.id))
                            class="text-accent font-medium cursor-pointer"
                        { (wo.doc_number) }
                    }
                }
                @if let (Some(pid), Some(pdoc)) = (wo.source_plan_id, wo.source_plan_doc.as_ref()) {
                    div class="flex flex-col gap-0.5" {
                        span class="text-xs text-muted" { "计划" }
                        span class="text-sm text-fg font-medium" {
                            a   href=(format!("/admin/mes/plans/{}", pid))
                                class="text-accent font-medium cursor-pointer"
                            { (pdoc) }
                        }
                    }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "产品" }
                    span class="text-sm text-fg font-medium truncate" title=(product_name) {
                        (product_name)
                    }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "班组" }
                    span class="text-sm text-fg font-medium" { "—" }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "批次数量" }
                    span class="text-sm text-fg font-medium" {
                        (crate::utils::fmt_qty(batch.batch_qty))
                    }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "完成/报废" }
                    span class="text-sm text-fg font-medium" {
                        span class="text-success" { (crate::utils::fmt_qty(batch.completed_qty)) }
                        " / "
                        span class="text-danger" { (crate::utils::fmt_qty(batch.scrap_qty)) }
                    }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "当前工序" }
                    span class="text-sm text-fg font-medium text-warn" { (current_step_display) }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "实际开始" }
                    span class="text-sm text-fg font-medium" {
                        ({
                            batch
                                .actual_start
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "—".to_string())
                        })
                    }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "实际结束" }
                    span class="text-sm text-fg font-medium text-muted" {
                        ({
                            batch
                                .actual_end
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "—".to_string())
                        })
                    }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "创建人" }
                    span class="text-sm text-fg font-medium" { (creator_name) }
                }
                div class="flex flex-col gap-0.5" {
                    span class="text-xs text-muted" { "创建时间" }
                    span class="text-sm text-fg font-medium" {
                        (batch.created_at.format("%Y-%m-%d %H:%M").to_string())
                    }
                }
            }
        }
        // ── 工序流转进度 (horizontal step dots) ──
        @if !routings.is_empty() {
            div class="bg-bg border border-border-soft rounded-xl p-5 overflow-hidden" {
                div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                    "工序流转进度"
                }
                div class="flex items-start relative py-2 gap-2" {
                    @for (i, r) in routings.iter().enumerate() {
                        @let brp = progress_map.get(&r.id);
                        @let is_completed = brp
                            .map(|b| {
                                b.status == abt_core::mes::enums::RoutingStatus::Completed
                            })
                            .unwrap_or(false);
                        @let is_active = r.step_no == batch.current_step;
                        @let step_str = r.step_no.to_string();
                        div class="flex flex-col items-center text-center shrink-0 min-w-[80px]" {
                            div class=({
                                    format!(
                                        "w-8 h-8 rounded-full grid place-items-center text-sm font-semibold {}",
                                        if is_completed {
                                            "bg-success text-white"
                                        } else if is_active {
                                            "bg-accent text-white"
                                        } else {
                                            "bg-surface text-muted"
                                        },
                                    )
                                })
                            {
                                @if is_completed { "\u{2713}" } @else { (step_str) }
                            }
                            div class="text-xs font-medium text-fg mt-2" { (r.process_name) }
                            div class="text-[11px] text-muted mt-1" {
                                @if let Some(p) = brp {
                                    @if is_completed || is_active {
                                        "完成 "
                                        (crate::utils::fmt_qty(p.completed_qty))
                                        @if p.defect_qty > rust_decimal::Decimal::ZERO {
                                            br;
                                            "不良 "
                                            (crate::utils::fmt_qty(p.defect_qty))
                                        }
                                    } @else { "待生产" }
                                } @else { "待生产" }
                            }
                        }
                        @if i < routings.len() - 1 {
                            div class=({
                                    format!(
                                        "flex-1 h-px mt-4 min-w-[20px] {}",
                                        if is_completed { "bg-success" } else { "bg-border-soft" },
                                    )
                                }) {}
                        }
                    }
                }
            }
        }
        // ── 报工记录 (matches prototype sub-section) ──
        div class="bg-bg border border-border-soft rounded-xl p-5 overflow-hidden" {
            div class="text-base font-semibold text-fg" { "报工记录" }
            @if reports.is_empty() {
                div class="text-center text-muted py-8" { "暂无报工记录" }
            } @else {
                div class="overflow-x-auto" {
                    table class="data-table w-full" {
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
                                    td {
                                        a   href=(format!("/admin/mes/reports/{}", r.id))
                                            class="text-accent font-medium cursor-pointer font-mono tabular-nums"
                                        { (r.doc_number) }
                                    }
                                    td { (routing_map.get(&r.routing_id).copied().unwrap_or("—")) }
                                    td {
                                        span
                                            class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-fg-2"
                                        { (shift_label(&r.shift)) }
                                    }
                                    td {
                                        ({
                                            user_map
                                                .get(&r.worker_id)
                                                .map(|s| s.as_str())
                                                .unwrap_or("—")
                                        })
                                    }
                                    td class="font-mono tabular-nums text-success" {
                                        (crate::utils::fmt_qty(r.completed_qty))
                                    }
                                    td class="font-mono tabular-nums text-danger" {
                                        (crate::utils::fmt_qty(r.defect_qty))
                                    }
                                    td { (defect_label(&r.defect_reason)) }
                                    td class="font-mono tabular-nums" {
                                        (crate::utils::fmt_qty(r.work_hours))
                                    }
                                    td class="text-muted text-[13px]" {
                                        (r.created_at.format("%Y-%m-%d %H:%M").to_string())
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // ── 状态变更记录 (matches prototype sub-section) ──
        div class="bg-bg border border-border-soft rounded-xl p-5 overflow-hidden" {
            div class="text-base font-semibold text-fg" { "状态变更记录" }
            table class="data-table w-full" {
                thead {
                    tr {
                        th { "时间" }
                        th class="!text-right" { "操作" }
                        th { "变更" }
                        th { "操作人" }
                        th { "备注" }
                    }
                }
                tbody {
                    // 创建记录
                    tr {
                        td class="text-muted text-[13px]" {
                            (batch.created_at.format("%Y-%m-%d %H:%M").to_string())
                        }
                        td {
                            span
                                class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted"
                            { "创建" }
                        }
                        td { "批次创建" }
                        td { (creator_name) }
                        td class="text-muted" { "工单下达自动生成" }
                    }
                    // 实际开始记录
                    @if let Some(start) = batch.actual_start {
                        tr {
                            td class="text-muted text-[13px]" {
                                (start.format("%Y-%m-%d %H:%M").to_string())
                            }
                            td {
                                span
                                    class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-accent-50 text-[var(--accent-active)]"
                                { "开始生产" }
                            }
                            td { "待生产 → 进行中" }
                            td { (creator_name) }
                            td class="text-muted" { "首道工序开始" }
                        }
                    }
                }
            }
        }
    }
}
}

