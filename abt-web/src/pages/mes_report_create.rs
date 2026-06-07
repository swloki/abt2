use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::shared::identity::UserService;

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

#[derive(Debug, Deserialize)]
pub struct ReportCreateQuery {
    pub batch_id: Option<i64>,
}

#[require_permission("MES", "write")]
pub async fn get_report_create(
    _path: ReportCreatePath, ctx: RequestContext,
    axum::extract::Query(query): axum::extract::Query<ReportCreateQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let user_svc = state.user_service();

    // Load work orders for dropdown
    let wo_filter = abt_core::mes::work_order::WorkOrderFilter {
        status: None, product_id: None, keyword: None, date_from: None, date_to: None,
    };
    let work_orders = wo_svc.list(&service_ctx, &mut conn, wo_filter, 1, 200).await?;
    let active_wos: Vec<_> = work_orders.items.into_iter().filter(|wo| wo.status != abt_core::mes::enums::WorkOrderStatus::Cancelled).collect();

    // If batch_id specified, load batch + routings
    let (batch, routings, wo) = if let Some(bid) = query.batch_id {
        let b = batch_svc.find_by_id(&service_ctx, &mut conn, bid).await?;
        let rs = batch_svc.list_routings(&service_ctx, &mut conn, b.work_order_id).await?;
        let w = wo_svc.find_by_id(&service_ctx, &mut conn, b.work_order_id).await?;
        (Some(b), rs, Some(w))
    } else {
        (None, vec![], None)
    };

    // Load workers (users)
    let workers_result = user_svc.list_users(&service_ctx, &mut conn, 1, 100).await?;
    let workers = workers_result.items;

    let content = report_create_page(&active_wos, batch.as_ref(), &routings, wo.as_ref(), &workers);
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

fn report_create_page(
    work_orders: &[abt_core::mes::work_order::WorkOrder],
    batch: Option<&abt_core::mes::production_batch::ProductionBatch>,
    routings: &[abt_core::mes::production_batch::WorkOrderRouting],
    wo: Option<&abt_core::mes::work_order::WorkOrder>,
    workers: &[abt_core::shared::identity::User],
) -> Markup {
    html! { div {
        div class="page-header" { h1 class="page-title" { "新建报工" } }
        form hx-post=(ReportCreatePath::PATH) hx-swap="none" {
            @if let Some(b) = batch {
                input type="hidden" name="batch_id" value=(b.id);
            }

            div class="form-section" {
                div class="form-section-title" { "基本信息" }
                div class="form-grid" {
                    div class="form-group" {
                        label class="form-label" { "工单 " span class="required" { "*" } }
                        select class="form-select" name="wo_id" required {
                            option value="" { "选择工单..." }
                            @for w in work_orders {
                                @let sel = wo.map_or(false, |cur| cur.id == w.id);
                                option value=(w.id) selected[sel] { (w.doc_number) }
                            }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "批次 " span class="required" { "*" } }
                        @if let Some(b) = batch {
                            input class="form-input" type="text" readonly value=(b.batch_no) style="background:var(--surface);color:var(--muted)";
                        } @else {
                            input class="form-input" type="number" name="batch_id" placeholder="输入批次ID" required;
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "工序 " span class="required" { "*" } }
                        @if !routings.is_empty() {
                            select class="form-select" name="step_no" required {
                                @for r in routings {
                                    @if r.status != abt_core::mes::enums::RoutingStatus::Completed {
                                        @let is_cur = batch.map_or(false, |b| b.current_step == r.step_no);
                                        option value=(r.step_no) selected[is_cur] { (r.step_no) " - " (r.process_name) }
                                    }
                                }
                            }
                        } @else {
                            input class="form-input" type="number" name="step_no" required;
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "班次 " span class="required" { "*" } }
                        div class="shift-toggle" {
                            button type="button" class="shift-btn active" onclick="this.classList.add('active');this.nextElementSibling.classList.remove('active');document.querySelector('input[name=shift]').value='1'" { "白班" }
                            button type="button" class="shift-btn" onclick="this.classList.add('active');this.previousElementSibling.classList.remove('active');document.querySelector('input[name=shift]').value='2'" { "夜班" }
                            input type="hidden" name="shift" value="1";
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "工人 " span class="required" { "*" } }
                        select class="form-select" name="worker_id" required {
                            option value="" { "选择工人..." }
                            @for w in workers {
                                option value=(w.user_id) { (w.display_name.as_deref().unwrap_or(&w.username)) }
                            }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "报工日期 " span class="required" { "*" } }
                        input class="form-input" type="date" name="report_date" value=(chrono::Local::now().format("%Y-%m-%d").to_string()) required;
                    }
                }
            }

            div class="form-section" {
                div class="form-section-title" { "生产数据" }
                div class="form-grid" {
                    div class="form-group" {
                        label class="form-label" { "完成数量 " span class="required" { "*" } }
                        input class="form-input" type="number" step="0.01" name="completed_qty" required;
                    }
                    div class="form-group" {
                        label class="form-label" { "不良数量" }
                        input class="form-input" type="number" step="0.01" name="defect_qty" value="0";
                    }
                    div class="form-group" {
                        label class="form-label" { "不良原因" }
                        select class="form-select" name="defect_reason" {
                            option value="" { "无" }
                            option value="1" { "物料不良" }
                            option value="2" { "设备故障" }
                            option value="3" { "操作失误" }
                            option value="4" { "工艺问题" }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "实际工时 (h)" }
                        input class="form-input" type="number" step="0.5" name="work_hours" required;
                    }
                    div class="form-group" {
                        label class="form-label" { "备注" }
                        textarea class="form-input" name="remark" style="height:80px" {};
                    }
                }
            }

            div class="form-actions" {
                a class="btn btn-default" href=(ReportListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "确认报工" }
            }
        }
    }}
}
