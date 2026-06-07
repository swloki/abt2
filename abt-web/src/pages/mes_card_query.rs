use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::WorkReportService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::{CardQueryPath, CardQuerySearchPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_card_query(_path: CardQueryPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = card_query_page();
    Ok(Html(admin_page(is_htmx, "流转卡查询", &claims, "production", CardQueryPath::PATH, "生产管理", None, content).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct CardSearchParams {
    pub q: Option<String>,
}

#[require_permission("MES", "read")]
pub async fn search_card(
    _path: CardQuerySearchPath,
    ctx: RequestContext,
    Query(params): Query<CardSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let query = match params.q {
        Some(ref q) if !q.trim().is_empty() => q.trim().to_string(),
        _ => {
            return Ok(Html(html! {
                div class="info-card" style="text-align:center;padding:var(--space-6);color:var(--muted)" {
                    "请输入流转卡序列号进行查询"
                }
            }.into_string()));
        }
    };

    let batch_svc = state.production_batch_service();
    let report_svc = state.work_report_service();
    let wo_svc = state.work_order_service();

    let batch = match batch_svc.find_by_card_sn(&service_ctx, &mut conn, query.clone()).await? {
        Some(b) => b,
        None => {
            return Ok(Html(html! {
                div class="info-card" style="text-align:center;padding:var(--space-6);color:var(--danger)" {
                    "未找到流转卡 \"" (query) "\" 对应的批次"
                }
            }.into_string()));
        }
    };

    let product_name = batch_svc.get_product_name(&mut conn, batch.product_id).await?.unwrap_or_default();
    let routings = batch_svc.list_routings(&service_ctx, &mut conn, batch.work_order_id).await?;
    let reports = report_svc.list_by_batch(&service_ctx, &mut conn, batch.id).await?;

    // Get work order doc_number via service
    let wo_doc_number = wo_svc.find_by_id(&service_ctx, &mut conn, batch.work_order_id)
        .await
        .map(|wo| wo.doc_number)
        .unwrap_or_default();

    let html_content = card_search_result(&batch, &product_name, &wo_doc_number, &routings, &reports);
    Ok(Html(html_content.into_string()))
}

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

fn card_query_page() -> Markup {
    html! { div {
        div class="page-header" {
            h1 class="page-title" { "流转卡查询" }
        }

        // Search box
        div class="info-card" style="text-align:center;padding:var(--space-8);margin-bottom:var(--space-6)" {
            div style="margin-bottom:var(--space-2);font-size:var(--text-lg);font-weight:600;color:var(--fg)" { "流转卡查询" }
            div style="color:var(--muted);margin-bottom:var(--space-4)" { "扫描或输入流转卡序列号，查看批次信息和生产进度" }
            form hx-get=(CardQuerySearchPath::PATH) hx-target="#card-result" hx-swap="innerHTML" hx-trigger="submit" {
                div style="display:flex;gap:var(--space-3);max-width:480px;margin:0 auto" {
                    input class="form-input" type="text" name="q" placeholder="扫描或输入卡号..." style="flex:1;font-size:var(--text-base)" autofocus;
                    button class="btn btn-primary" type="submit" { "查询" }
                }
            }
        }

        // Result area
        div id="card-result" {}
    }}
}

fn card_search_result(
    batch: &abt_core::mes::production_batch::ProductionBatch,
    product_name: &str,
    wo_doc_number: &str,
    routings: &[abt_core::mes::production_batch::WorkOrderRouting],
    reports: &[abt_core::mes::work_report::WorkReport],
) -> Markup {
    let (status_label, status_cls) = batch_status_label(&batch.status);
    let total_steps = routings.len() as i32;

    let current_step_display = if batch.current_step == 0 {
        "未开始".to_string()
    } else {
        let step_name = routings.iter()
            .find(|r| r.step_no == batch.current_step)
            .map(|r| r.process_name.as_str())
            .unwrap_or("—");
        format!("{}/{} {}", batch.current_step, total_steps, step_name)
    };

    let actual_start_str = batch.actual_start
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string());

    html! {
        // Result header
        div class="info-card" style="margin-bottom:var(--space-4)" {
            div style="display:flex;align-items:center;gap:var(--space-3);margin-bottom:var(--space-4)" {
                span class="mono" style="font-size:var(--text-lg);font-weight:600" { (batch.card_sn) }
                span class=(format!("status-pill {status_cls}")) { (status_label) }
            }
            div style="color:var(--muted);font-size:var(--text-sm)" {
                "批次 " span class="mono" { (batch.batch_no) }
                " · 工单 " a href=(format!("/admin/mes/orders/{}", batch.work_order_id)) style="color:var(--accent)" { (wo_doc_number) }
            }
        }

        // Info grid
        div class="info-card" style="margin-bottom:var(--space-4)" {
            div class="info-grid" {
                div class="info-item" { label { "产品" } span { (product_name) } }
                div class="info-item" { label { "批次数量" } span class="mono" { (crate::utils::fmt_qty(batch.batch_qty)) } }
                div class="info-item" { label { "已完成 / 报废" } span class="mono" { (crate::utils::fmt_qty(batch.completed_qty)) " / " (crate::utils::fmt_qty(batch.scrap_qty)) } }
                div class="info-item" { label { "当前工序" } span { (current_step_display) } }
                div class="info-item" { label { "实际开始" } span { (actual_start_str) } }
                div class="info-item" { label { "状态" } span class=(format!("status-pill {status_cls}")) { (status_label) } }
            }
        }

        // Flow progress - workflow steps
        @if !routings.is_empty() {
            div class="info-card" style="margin-bottom:var(--space-4)" {
                div style="font-weight:600;margin-bottom:var(--space-3)" { "工序进度" }
                div class="workflow-steps" {
                    @for (i, routing) in routings.iter().enumerate() {
                        @let is_completed = routing.status == abt_core::mes::enums::RoutingStatus::Completed;
                        @let is_current = routing.step_no == batch.current_step;

                        div class="wf-step" {
                            @if is_completed {
                                div class="wf-step-dot wf-step-done" { "✓" }
                            } @else if is_current {
                                div class="wf-step-dot wf-step-active" { (routing.step_no) }
                            } @else {
                                div class="wf-step-dot wf-step-pending" { (routing.step_no) }
                            }
                            div class="wf-step-name" { (routing.process_name) }
                            div class="wf-step-info" {
                                @if is_completed || is_current {
                                    span style="font-size:var(--text-xs);color:var(--muted)" {
                                        "完成 " (crate::utils::fmt_qty(routing.completed_qty))
                                        @if routing.defect_qty > rust_decimal::Decimal::ZERO {
                                            " / 不良 " (crate::utils::fmt_qty(routing.defect_qty))
                                        }
                                    }
                                }
                            }
                        }
                        @if i < routings.len() - 1 {
                            div class="wf-step-bar" {}
                        }
                    }
                }
            }
        }

        // Work report detail table
        @if !reports.is_empty() {
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead { tr {
                            th { "报工单号" }
                            th { "工序" }
                            th { "班次" }
                            th { "报工日期" }
                            th class="num-right" { "完成" }
                            th class="num-right" { "不良" }
                            th class="num-right" { "工时" }
                            th { "备注" }
                        }}
                        tbody {
                            @for report in reports {
                                tr {
                                    td class="mono" {
                                        a href=(format!("/admin/mes/reports/{}", report.id)) style="color:var(--accent)" { (report.doc_number) }
                                    }
                                    td {
                                        (routings.iter().find(|r| r.id == report.routing_id)
                                            .map(|r| r.process_name.as_str())
                                            .unwrap_or("—"))
                                    }
                                    td {
                                        @if report.shift == abt_core::mes::enums::ShiftType::Day { "白班" }
                                        @else { "夜班" }
                                    }
                                    td { (report.report_date) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(report.completed_qty)) }
                                    td class="num-right mono" {
                                        @if report.defect_qty > rust_decimal::Decimal::ZERO {
                                            span style="color:var(--danger)" { (crate::utils::fmt_qty(report.defect_qty)) }
                                        } @else {
                                            "0"
                                        }
                                    }
                                    td class="num-right mono" { (crate::utils::fmt_qty(report.work_hours)) "h" }
                                    td style="max-width:120px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" { (report.remark) }
                                }
                            }
                        }
                    }
                }
            }
        } @else {
            div class="info-card" style="text-align:center;padding:var(--space-4);color:var(--muted)" {
                "暂无报工记录"
            }
        }
    }
}
