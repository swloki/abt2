use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::{BatchStatus, RoutingStatus, WorkOrderStatus};
use abt_core::mes::production_batch::repo::BatchRoutingProgressRepo;
use abt_core::mes::production_batch::{ProductionBatchService, WorkOrderRouting};
use abt_core::mes::work_order::{WorkOrderFilter, WorkOrderService};
use abt_core::shared::identity::UserService;

use crate::components::entity_picker::{self, EntityPickerConfig, EntityPickerItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::{
    ReportBatchSelectedPath, ReportCreatePath, ReportListPath, ReportSearchBatchPath,
    ReportSearchWoPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query / Form ──

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
pub struct SearchParams {
    pub q: Option<String>,
    pub work_order_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BatchSelectedQuery {
    pub batch_id: i64,
}

// ── GET /reports/create ──

#[require_permission("WORK_ORDER", "create")]
pub async fn get_report_create(
    _path: ReportCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        claims,
        ..
    } = ctx;
    let user_svc = state.user_service();

    // 工人列表
    let workers = user_svc
        .list_users(&ctx.service_ctx, &mut conn, 1, 100)
        .await?
        .items;

    let doc_number = format!("WR-{}", chrono::Local::now().format("%Y-%m-%d"));
    let content = report_create_page(&workers, &doc_number);
    Ok(Html(
        admin_page(
            is_htmx,
            "新建报工",
            &claims,
            "production",
            ReportCreatePath::PATH,
            "生产管理",
            Some(ReportListPath::PATH),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

// ── HTMX: 搜索工单 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn search_wo(
    _path: ReportSearchWoPath,
    ctx: RequestContext,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wo_svc = state.work_order_service();
    let kw = params.q.as_deref().unwrap_or("").trim().to_string();

    let no_filter = WorkOrderFilter {
        status: None,
        product_id: None,
        keyword: None,
        date_from: None,
        date_to: None,
    };
    let mk = |st: WorkOrderStatus, keyword: String| WorkOrderFilter {
        status: Some(st),
        keyword: if keyword.is_empty() { None } else { Some(keyword) },
        ..no_filter.clone()
    };

    let released = wo_svc
        .list(&service_ctx, &mut conn, mk(WorkOrderStatus::Released, kw.clone()), 1, 50)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let in_prod = wo_svc
        .list(&service_ctx, &mut conn, mk(WorkOrderStatus::InProduction, kw), 1, 50)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let wos: Vec<_> = released.into_iter().chain(in_prod).collect();

    let mut pnames: HashMap<i64, String> = HashMap::new();
    let pids: HashSet<i64> = wos.iter().map(|w| w.product_id).collect();
    for pid in pids {
        if let Ok(Some(n)) = wo_svc.get_product_name(&mut conn, pid).await {
            pnames.insert(pid, n);
        }
    }

    let items: Vec<EntityPickerItem> = wos
        .iter()
        .map(|w| {
            let p = pnames.get(&w.product_id).map(|s| s.as_str()).unwrap_or("—");
            EntityPickerItem::new(w.id, format!("{} · {}", w.doc_number, p))
                .sub(format!("计划 {} 件", crate::utils::fmt_qty(w.planned_qty)))
        })
        .collect();

    Ok(Html(entity_picker::entity_picker_results(&items).into_string()))
}

// ── HTMX: 搜索批次（按工单过滤）──

#[require_permission("WORK_ORDER", "read")]
pub async fn search_batch(
    _path: ReportSearchBatchPath,
    ctx: RequestContext,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();

    let wo_id = match params.work_order_id {
        Some(id) if id > 0 => id,
        _ => {
            let items = vec![];
            return Ok(Html(entity_picker::entity_picker_results(&items).into_string()));
        }
    };

    let batches = batch_svc
        .list_by_work_order(&service_ctx, &mut conn, wo_id)
        .await
        .unwrap_or_default();
    let wo = wo_svc.find_by_id(&service_ctx, &mut conn, wo_id).await.ok();
    let product_name = if let Some(ref w) = wo {
        wo_svc.get_product_name(&mut conn, w.product_id).await.ok().flatten().unwrap_or_else(|| "—".into())
    } else { "—".into() };

    let kw = params.q.as_deref().unwrap_or("").trim().to_lowercase();

    let items: Vec<EntityPickerItem> = batches
        .iter()
        .filter(|b| {
            kw.is_empty()
                || b.batch_no.to_lowercase().contains(&kw)
                || b.card_sn.to_lowercase().contains(&kw)
        })
        .filter(|b| !matches!(b.status, BatchStatus::Cancelled | BatchStatus::Completed))
        .map(|b| {
            EntityPickerItem::new(b.id, format!("{} · {}件 · {}", b.batch_no, crate::utils::fmt_qty(b.batch_qty), product_name))
                .sub(format!("流转卡 {} · 当前第 {} 步", b.card_sn, b.current_step))
        })
        .collect();

    Ok(Html(entity_picker::entity_picker_results(&items).into_string()))
}

// ── HTMX: 批次选中后级联 — 返回工单信息 + 工序下拉 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn batch_selected(
    _path: ReportBatchSelectedPath,
    ctx: RequestContext,
    Query(params): Query<BatchSelectedQuery>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let batch_svc = state.production_batch_service();

    let batch = batch_svc
        .find_by_id(&service_ctx, &mut conn, params.batch_id)
        .await?;
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, batch.work_order_id)
        .await?;
    let completed: HashSet<i64> = BatchRoutingProgressRepo::list_by_batch(&mut *conn, params.batch_id)
        .await?
        .into_iter()
        .filter(|p| p.status == RoutingStatus::Completed)
        .map(|p| p.routing_id)
        .collect();

    Ok(Html(
        batch_cascade_fragment(batch.work_order_id, &batch, &routings, &completed).into_string(),
    ))
}

// ── POST /reports/create ──

#[require_permission("WORK_ORDER", "create")]
pub async fn create_report(
    _path: ReportCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReportCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let req = abt_core::mes::production_batch::StepConfirmationReq {
        step_no: form.step_no,
        worker_id: form.worker_id,
        shift: form.shift,
        completed_qty: form.completed_qty,
        defect_qty: form.defect_qty,
        defect_reason: form
            .defect_reason
            .and_then(abt_core::mes::enums::DefectReason::from_i16),
        work_hours: form.work_hours.unwrap_or_default(),
        report_date: form.report_date,
        remark: form.remark,
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    svc.confirm_routing_step(&service_ctx, &mut tx, form.batch_id, form.step_no, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", ReportListPath::PATH)
        .body(axum::body::Body::empty())
        .unwrap())
}

// ── Page content ──

fn report_create_page(
    workers: &[abt_core::shared::identity::User],
    doc_number: &str,
) -> Markup {
    let wo_picker = EntityPickerConfig {
        modal_id: "wo-picker",
        title: "选择工单",
        search_label: "工单号 / 产品名",
        search_placeholder: "输入关键词搜索…",
        search_path: ReportSearchWoPath::PATH,
        search_param: "q",
        target_id: "work_order_id",
        display_id: "wo-display",
        event_name: "woSelected",
        extra_include: None,
    };
    let batch_picker = EntityPickerConfig {
        modal_id: "batch-picker",
        title: "选择批次",
        search_label: "批次号 / 流转卡号",
        search_placeholder: "输入关键词搜索…",
        search_path: ReportSearchBatchPath::PATH,
        search_param: "q",
        target_id: "batch_id",
        display_id: "batch-display",
        event_name: "batchSelected",
        extra_include: Some("#work_order_id"),
    };

    html! { div {
        div class="page-header" { h1 class="page-title" { "新建报工" } }
        form hx-post=(ReportCreatePath::PATH) hx-swap="none" id="report-form" {

            // === 基本信息 ===
            div class="form-section" {
                div class="form-section-title" { "基本信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label class="form-label" { "报工单号" }
                        input class="form-input" type="text" readonly value=(doc_number)
                            style="background:var(--surface);color:var(--text-muted)";
                    }
                    // 工单：搜索选择框
                    (entity_picker::entity_picker_field(
                        "work_order_id", "work_order_id", "wo-display", "wo-picker",
                        "工单", false, "点击选择工单…",
                    ))
                    // 批次：搜索选择框
                    (entity_picker::entity_picker_field(
                        "batch_id", "batch_id", "batch-display", "batch-picker",
                        "批次", false, "选择工单后可选…",
                    ))
                }
                // 批次选中后级联：工序下拉
                div id="batch-cascade"
                    hx-get=(ReportBatchSelectedPath::PATH)
                    hx-trigger="batchSelected from:body"
                    hx-target="this"
                    hx-swap="outerHTML"
                    hx-include="#batch_id" {}
            }

            // === 报工数据 ===
            div class="form-section" {
                div class="form-section-title" { "报工数据" }
                div class="form-grid" {
                    div class="form-field" {
                        label class="form-label" { "班次 " span class="required" { "*" } }
                        div class="shift-toggle" {
                            button type="button" class="shift-btn active"
                                _="on click take .active from .shift-btn then put '1' into (closest .shift-toggle)'s first input's value" { "白班" }
                            button type="button" class="shift-btn"
                                _="on click take .active from .shift-btn then put '2' into (closest .shift-toggle)'s first input's value" { "夜班" }
                            input type="hidden" name="shift" value="1";
                        }
                    }
                    div class="form-field" {
                        label class="form-label" { "工人 " span class="required" { "*" } }
                        select class="form-select" name="worker_id" required {
                            option value="" { "请选择工人" }
                            @for w in workers {
                                option value=(w.user_id) { (w.display_name.as_deref().unwrap_or(&w.username)) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="form-label" { "报工日期 " span class="required" { "*" } }
                        input class="form-input" type="date" name="report_date"
                            value=(chrono::Local::now().format("%Y-%m-%d").to_string()) required;
                    }
                    div class="form-field" {
                        label class="form-label" { "完成数量 " span class="required" { "*" } }
                        input class="form-input" type="number" placeholder="0" min="0" name="completed_qty" required;
                    }
                    div class="form-field" {
                        label class="form-label" { "不良数量" }
                        input class="form-input" type="number" placeholder="0" min="0" name="defect_qty" value="0";
                    }
                    div class="form-field" {
                        label class="form-label" { "不良原因" }
                        select class="form-select" name="defect_reason" {
                            option value="" { "—" }
                            option value="1" { "物料不良" }
                            option value="2" { "设备故障" }
                            option value="3" { "操作失误" }
                            option value="4" { "工艺问题" }
                        }
                    }
                    div class="form-field" {
                        label class="form-label" { "实际工时 (h)" }
                        input class="form-input" type="number" placeholder="0" step="0.5" min="0" name="work_hours";
                    }
                }
                div style="margin-top:var(--space-4)" {
                    label class="form-label" { "备注" }
                    textarea class="form-textarea" name="remark" placeholder="报工备注…"
                        style="margin-top:var(--space-1)" {};
                }
            }

            div class="form-actions" {
                a class="btn btn-default" href=(ReportListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "确认报工" }
            }
        }

        // ── 弹窗 ──
        (entity_picker::entity_picker_modal(&wo_picker))
        (entity_picker::entity_picker_modal(&batch_picker))
    }}
}

// ── HTMX fragments ──

fn batch_cascade_fragment(
    work_order_id: i64,
    batch: &abt_core::mes::production_batch::ProductionBatch,
    routings: &[WorkOrderRouting],
    completed: &HashSet<i64>,
) -> Markup {
    html! {
        div id="batch-cascade" {
            // 隐藏的 work_order_id（报工提交时携带）
            input type="hidden" name="work_order_id" value=(work_order_id);
            div class="form-grid" {
                div class="form-field" {
                    label class="form-label" { "工序 " span class="required" { "*" } }
                    @if routings.is_empty() {
                        select class="form-select" name="step_no" disabled {
                            option value="" { "该工单暂无工序路线" }
                        }
                    } @else {
                        select class="form-select" name="step_no" required {
                            option value="" { "请选择工序" }
                            @for r in routings {
                                @if !completed.contains(&r.id) {
                                    @let is_cur = batch.current_step == r.step_no;
                                    @let tag = if is_cur { " [当前工序]" } else { "" };
                                    option value=(r.step_no) selected[is_cur] {
                                        (r.step_no) " - " (r.process_name) (tag)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
