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
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub defect_reason: Option<i16>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub work_hours: Option<rust_decimal::Decimal>,
    pub report_date: chrono::NaiveDate,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReportCreateQuery {
    pub batch_id: Option<i64>,
}

#[require_permission("WORK_ORDER", "create")]
pub async fn get_report_create(
    _path: ReportCreatePath, ctx: RequestContext,
    axum::extract::Query(query): axum::extract::Query<ReportCreateQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
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

    // Load product names for work orders
    let mut wo_product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for w in &active_wos {
        if let Ok(Some(name)) = wo_svc.get_product_name(&mut conn, w.product_id).await {
            wo_product_names.insert(w.id, name);
        }
    }

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

    // Generate doc number for display (WR-yyyy-mm-NNNNN)
    let doc_number = format!("WR-{}", chrono::Local::now().format("%Y-%m-%d"));

    let content = report_create_page(&active_wos, &wo_product_names, batch.as_ref(), &routings, wo.as_ref(), &workers, &doc_number);
    Ok(Html(admin_page(is_htmx, "新建报工", &claims, "production", ReportCreatePath::PATH, "生产管理", Some(ReportListPath::PATH), content, &nav_filter).into_string()))

}

#[require_permission("WORK_ORDER", "create")]
pub async fn create_report(
    _path: ReportCreatePath, ctx: RequestContext, axum::Form(form): axum::Form<ReportCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let req = abt_core::mes::production_batch::StepConfirmationReq {
        step_no: form.step_no,
        worker_id: form.worker_id,
        shift: form.shift,
        completed_qty: form.completed_qty,
        defect_qty: form.defect_qty,
        defect_reason: form.defect_reason.and_then(abt_core::mes::enums::DefectReason::from_i16),
        work_hours: form.work_hours.unwrap_or_default(),
        report_date: form.report_date,
        remark: form.remark,
    };
    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    svc.confirm_routing_step(&service_ctx, &mut tx, form.batch_id, form.step_no, req).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(axum::response::Response::builder().header("HX-Redirect", ReportListPath::PATH).body(axum::body::Body::empty()).unwrap())
}

fn report_create_page(
    work_orders: &[abt_core::mes::work_order::WorkOrder],
    wo_product_names: &std::collections::HashMap<i64, String>,
    batch: Option<&abt_core::mes::production_batch::ProductionBatch>,
    routings: &[abt_core::mes::production_batch::WorkOrderRouting],
    wo: Option<&abt_core::mes::work_order::WorkOrder>,
    workers: &[abt_core::shared::identity::User],
    doc_number: &str,
) -> Markup {
    // Find current routing's unit_price
    let current_step = batch.map(|b| b.current_step);
    let current_routing = routings.iter().find(|r| Some(r.step_no) == current_step);
    let unit_price = current_routing.and_then(|r| r.unit_price).unwrap_or(rust_decimal::Decimal::ZERO);
    let unit_price_display = if unit_price == rust_decimal::Decimal::ZERO {
        "—".to_string()
    } else {
        format!("¥{}", crate::utils::fmt_qty(unit_price))
    };

    html! { div {
        div class="page-header" { h1 class="page-title" { "新建报工" } }
        form hx-post=(ReportCreatePath::PATH) hx-swap="none" {
            @if let Some(b) = batch {
                input type="hidden" name="batch_id" value=(b.id);
            }

            // === 基本信息 ===
            div class="form-section" {
                div class="form-section-title" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2"/><rect x="8" y="2" width="8" height="4" rx="1"/></svg>"#))
                    "基本信息"
                }
                div class="form-grid" {
                    div class="form-group" {
                        label class="form-label" { "报工单号" }
                        input class="form-input" type="text" readonly value=(doc_number) style="background:var(--surface);color:var(--muted)";
                    }
                    div class="form-group" {
                        label class="form-label" { "工单 " span class="required" { "*" } }
                        select class="form-select" name="wo_id" required {
                            option value="" { "请选择工单" }
                            @for w in work_orders {
                                @let sel = wo.is_some_and(|cur| cur.id == w.id);
                                @let label = wo_product_names.get(&w.id).map_or(w.doc_number.clone(), |name| format!("{} ({})", w.doc_number, name));
                                option value=(w.id) selected[sel] { (label) }
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
                                        @let is_cur = batch.is_some_and(|b| b.current_step == r.step_no);
                                        @let cur_tag = if is_cur { " [当前工序]" } else { "" };
                                        option value=(r.step_no) selected[is_cur] { (r.step_no) " - " (r.process_name) (cur_tag) }
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
                            button type="button" class="shift-btn active" _="on click take .active from .shift-btn then put '1' into (closest .shift-toggle)'s first input's value" { "白班" }
                            button type="button" class="shift-btn" _="on click take .active from .shift-btn then put '2' into (closest .shift-toggle)'s first input's value" { "夜班" }
                            input type="hidden" name="shift" value="1";
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "工人 " span class="required" { "*" } }
                        select class="form-select" name="worker_id" required {
                            option value="" { "请选择工人" }
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

            // === 生产数据 ===
            div class="form-section" {
                div class="form-section-title" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20h9M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>"#))
                    "生产数据"
                }
                div class="form-grid" {
                    div class="form-group" {
                        label class="form-label" { "完成数量 " span class="required" { "*" } }
                        input class="form-input" type="number" placeholder="0" min="0" name="completed_qty" required;
                    }
                    div class="form-group" {
                        label class="form-label" { "不良数量" }
                        input class="form-input" type="number" placeholder="0" min="0" name="defect_qty" value="0";
                    }
                    div class="form-group" {
                        label class="form-label" { "不良原因" }
                        select class="form-select" name="defect_reason" {
                            option value="" { "—" }
                            option value="1" { "物料不良 (MaterialDefect)" }
                            option value="2" { "设备故障 (EquipmentFault)" }
                            option value="3" { "操作失误 (OperatorError)" }
                            option value="4" { "工艺问题 (ProcessIssue)" }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "实际工时 (h)" }
                        input class="form-input" type="number" placeholder="0" step="0.5" min="0" name="work_hours";
                    }
                    div class="form-group" {
                        label class="form-label" { "计件单价" }
                        input class="form-input" type="text" readonly value=(unit_price_display) style="background:var(--surface);color:var(--muted)";
                    }
                    div class="form-group" {
                        label class="form-label" { "预计工资" }
                        div class="wage-display" {
                            div class="wage-amount" id="wageAmount" { "¥0.00" }
                            div class="wage-label" { "完成数量 × 计件单价" }
                        }
                    }
                }
                div style="margin-top:var(--space-4)" {
                    label class="form-label" { "备注" }
                    textarea class="form-textarea" name="remark" placeholder="报工备注…" style="margin-top:var(--space-1)" {};
                }
            }

            div class="form-actions" {
                a class="btn btn-default" href=(ReportListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "确认报工" }
            }
        }
        // 预计工资实时计算
        (maud::PreEscaped(format!("<script>document.querySelector('input[name=completed_qty]').addEventListener('input',function(e){{var q=parseFloat(e.target.value)||0;var p={unit_price};document.querySelector('#wageAmount').textContent='¥'+(q*p).toFixed(2)}})</script>")))
    }}
}
