use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::enums::{BatchStatus, ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{ProductionBatch, ProductionBatchService, SplitReq, WorkOrderRouting};
use abt_core::mes::work_report::{ReportListFilter, ReportListItem, WorkReportService};
use abt_core::mes::work_order::{WorkOrder, WorkOrderService};
use abt_core::shared::audit_log::{AuditLog, AuditLogQuery, AuditLogService};
use abt_core::shared::enums::audit::AuditAction;

use crate::components::detail::{detail_tabs, tab_panel};
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{
    OrderCancelPath, OrderClosePath, OrderDetailPath, OrderListPath, OrderReleasePath,
    OrderSplitPath, OrderUnreleasePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn wo_status_label(s: &WorkOrderStatus) -> (&'static str, &'static str, &'static str) {
    use WorkOrderStatus::*;
    match s {
        Draft => ("待计划", "rgba(0,0,0,0.04)", "var(--muted)"),
        Planned => ("已计划", "rgba(22,119,255,0.08)", "var(--accent)"),
        Released => ("已下达", "rgba(82,196,26,0.08)", "var(--success)"),
        InProduction => ("生产中", "rgba(250,173,20,0.08)", "#faad14"),
        Closed => ("已关闭", "rgba(114,46,209,0.08)", "#722ed1"),
        Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

fn status_pill(label: &str, bg: &str, color: &str) -> Markup {
    html! {
        span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 10px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{bg};color:{color}")) {
            (label)
        }
    }
}

fn batch_status_pill(s: BatchStatus) -> Markup {
    let (l, bg, c) = match s {
        BatchStatus::Pending => ("待生产", "rgba(0,0,0,0.04)", "var(--muted)"),
        BatchStatus::InProgress => ("进行中", "rgba(22,119,255,0.08)", "var(--accent)"),
        BatchStatus::Suspended => ("已暂停", "rgba(250,140,22,0.08)", "#fa8c16"),
        BatchStatus::PendingReceipt => ("待入库", "rgba(22,119,255,0.08)", "var(--accent)"),
        BatchStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        BatchStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    };
    status_pill(l, bg, c)
}

fn shift_label(s: ShiftType) -> &'static str {
    match s {
        ShiftType::Day => "白班",
        ShiftType::Night => "夜班",
    }
}

fn audit_action_label(a: AuditAction) -> &'static str {
    match a {
        AuditAction::Create => "创建",
        AuditAction::Update => "更新",
        AuditAction::Delete => "删除",
        AuditAction::Transition => "状态流转",
    }
}

fn fmt_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M").to_string()
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_detail(
    path: OrderDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        claims,
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();
    let report_svc = state.work_report_service();
    let audit_svc = state.audit_log_service();

    let order = wo_svc
        .find_by_id(&service_ctx, &mut conn, path.id)
        .await?;

    let product_name = wo_svc
        .get_product_name(&mut conn, order.product_id)
        .await?
        .unwrap_or_default();

    // 工序明细
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, order.id)
        .await
        .unwrap_or_default();

    // 生产批次
    let batches = batch_svc
        .list_by_work_order(&service_ctx, &mut conn, order.id)
        .await
        .unwrap_or_default();

    // 报工记录
    let reports = report_svc
        .list(
            &service_ctx,
            &mut conn,
            ReportListFilter {
                work_order_id: Some(order.id),
                ..Default::default()
            },
            1,
            100,
        )
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    // 操作日志
    let audit_logs = audit_svc
        .query_logs(
            &service_ctx,
            &mut conn,
            AuditLogQuery {
                entity_type: Some("WorkOrder".to_string()),
                entity_id: Some(order.id),
                ..Default::default()
            },
            1,
            50,
        )
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    let content = order_detail_page(
        &order, &product_name, &routings, &batches, &reports, &audit_logs,
    );
    let page_html = admin_page(
        is_htmx,
        "工单详情",
        &claims,
        "production",
        &format!("/admin/mes/orders/{}", path.id),
        "生产管理",
        Some(OrderListPath::PATH),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn release_order(
    path: OrderReleasePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

    if order.status == WorkOrderStatus::Released {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.release(&service_ctx, &mut conn, path.order_id, order.version)
        .await?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn unrelease_order(
    path: OrderUnreleasePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

    // 幂等：已是草稿则直接重定向
    if order.status == WorkOrderStatus::Draft {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.unrelease(&service_ctx, &mut conn, path.order_id, order.version)
        .await?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn close_order(
    path: OrderClosePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

    if order.status == WorkOrderStatus::Closed {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.close(&service_ctx, &mut conn, path.order_id, order.version)
        .await?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn cancel_order(
    path: OrderCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

    if order.status == WorkOrderStatus::Cancelled {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.cancel(&service_ctx, &mut conn, path.order_id, order.version)
        .await?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct SplitForm {
    pub split_qty: String,
}

/// 拆批：从工单创建额外的生产批次
#[require_permission("WORK_ORDER", "update")]
pub async fn split_order(
    path: OrderSplitPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SplitForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch_svc = state.production_batch_service();

    let split_qty = form.split_qty.parse::<rust_decimal::Decimal>()
        .map_err(|_| abt_core::shared::types::DomainError::validation("数量格式错误"))?;

    if split_qty <= rust_decimal::Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::validation("拆批数量必须大于 0").into());
    }

    batch_svc.split_work_order(
        &service_ctx, &mut conn, path.order_id,
        vec![SplitReq { batch_qty: split_qty, team_id: None }],
    ).await?;

    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page ──

#[allow(clippy::too_many_arguments)]
fn order_detail_page(
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
    batches: &[ProductionBatch],
    reports: &[ReportListItem],
    audit_logs: &[AuditLog],
) -> Markup {
    let (status_label, status_bg, status_color) = wo_status_label(&order.status);
    let routing_tab_label = format!("工序明细 {}", routings.len());

    html! {
        div {
            // 返回
            a class="back-link" href=(OrderListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回工单列表"
            }

            // Detail Header
            div class="detail-header" {
                div class="detail-title-row" {
                    div class="detail-doc-no mono" {
                        span { (order.doc_number) }
                        (status_pill(status_label, status_bg, status_color))
                    }
                    div class="page-actions" {
                        @if matches!(order.status, WorkOrderStatus::Released) {
                            button class="btn btn-default" type="button" _="on click add .is-open to #unrelease-dialog" {
                                "反下达"
                            }
                            button class="btn btn-default"
                                hx-post=(OrderClosePath { order_id: order.id }.to_string())
                                hx-confirm="确认关闭此工单？"
                                hx-disabled-elt="this" {
                                (icon::check_circle_icon("w-4 h-4"))
                                "关闭工单"
                            }
                        }
                        @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned) {
                            button class="btn btn-primary"
                                hx-post=(OrderReleasePath { order_id: order.id }.to_string())
                                hx-confirm="确认下达此工单？下达后将开始生产。"
                                hx-disabled-elt="this" {
                                (icon::rocket_icon("w-4 h-4"))
                                "下达工单"
                            }
                        }
                        @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned | WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
                            button class="btn btn-danger"
                                hx-post=(OrderCancelPath { order_id: order.id }.to_string())
                                hx-confirm="确认取消此工单？取消后不可恢复。"
                                hx-disabled-elt="this" {
                                (icon::x_icon("w-4 h-4"))
                                "取消工单"
                            }
                        }
                    }
                }

                // 副标题行
                div class="detail-sub-row" {
                    span { (product_name) }
                    span class="sep" { "|" }
                    span class="mono" { (crate::utils::fmt_qty(order.planned_qty)) " 件" }
                    @if order.completed_qty > rust_decimal::Decimal::ZERO {
                        span class="sep" { "|" }
                        span class="mono" style="color:var(--success)" { "完成 " (crate::utils::fmt_qty(order.completed_qty)) }
                    }
                    @if order.scrap_qty > rust_decimal::Decimal::ZERO {
                        span class="sep" { "|" }
                        span class="mono" style="color:var(--danger)" { "报废 " (crate::utils::fmt_qty(order.scrap_qty)) }
                    }
                    span class="sep" { "|" }
                    span { "—" }
                    @if let Some(so) = order.source_so_doc.as_ref() {
                        span class="sep" { "|" }
                        span { "销售订单: " (so) }
                        @if let Some(c) = order.source_customer.as_ref() {
                            span class="muted" { " (" (c) ")" }
                        }
                    }
                    @if let Some(pdoc) = order.source_plan_doc.as_ref() {
                        span class="sep" { "|" }
                        span { "生产计划: " }
                        @if let Some(pid) = order.source_plan_id {
                            a class="link-cell" href=(format!("/admin/mes/plans/{pid}")) { (pdoc) }
                        } @else { span { (pdoc) } }
                    }
                }
            }

            // Tabs
            (detail_tabs("info", &[
                ("info", "工单信息"),
                ("routing", &routing_tab_label),
                ("batches", &format!("生产批次 {}", batches.len())),
                ("reports", &format!("报工记录 {}", reports.len())),
                ("log", "操作日志"),
            ]))

            (tab_panel("info", true, tab_info(order, product_name, routings.len())))
            (tab_panel("routing", false, tab_routing(routings)))
            (tab_panel("batches", false, tab_batches(batches, routings, order)))
            (tab_panel("reports", false, tab_reports(reports)))
            (tab_panel("log", false, tab_log(audit_logs)))

            // 反下达对话框
            @if matches!(order.status, WorkOrderStatus::Released) {
                div class="modal-overlay" id="unrelease-dialog" {
                    div class="modal modal-sm" {
                        div class="modal-head" {
                            h2 { "确认反下达？" }
                        }
                        div class="modal-body" {
                            p class="modal-desc" {
                                "反下达将回退工单到 "
                                strong { "草稿" }
                                " 状态，同时取消领料单、释放库存预留、软删除生产批次（若有报工记录则无法反下达）。此操作不可撤销。"
                            }
                        }
                        div class="modal-foot" {
                            button class="btn btn-default" type="button" _="on click remove .is-open from #unrelease-dialog" {
                                "取消"
                            }
                            button class="btn btn-danger"
                                hx-post=(OrderUnreleasePath { order_id: order.id }.to_string())
                                hx-confirm="确认执行反下达？"
                                hx-disabled-elt="this" {
                                "确认反下达"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Tab Panels ──

fn tab_info(order: &WorkOrder, product_name: &str, routing_count: usize) -> Markup {
    let (sl, sb, sc) = wo_status_label(&order.status);
    html! {
        div class="bento-grid" {
            div class="bento-half" {
                div class="info-section-title" { "基础信息" }
                div class="bento-sub-grid" {
                    div class="info-item" { span class="info-label" { "工单编号" } span class="info-value mono" { (order.doc_number) } }
                    div class="info-item" { span class="info-label" { "产品" } span class="info-value" { (product_name) } }
                    div class="info-item" { span class="info-label" { "计划数量" } span class="info-value mono" { (crate::utils::fmt_qty(order.planned_qty)) } }
                    div class="info-item" { span class="info-label" { "状态" } span class="info-value" { (status_pill(sl, sb, sc)) } }
                    div class="info-item" { span class="info-label" { "版本号" } span class="info-value mono" { "v"(order.version) } }
                    div class="info-item" { span class="info-label" { "计划开始" } span class="info-value mono" { (order.scheduled_start) } }
                    div class="info-item" { span class="info-label" { "计划结束" } span class="info-value mono" { (order.scheduled_end) } }
                    div class="info-item" { span class="info-label" { "创建人" } span class="info-value mono" { "#" (order.operator_id) } }
                }
            }
            div class="bento-half" {
                div class="info-section-title" { "生产配置" }
                div class="bento-sub-grid" {
                    div class="info-item" {
                        span class="info-label" { "BOM 快照" }
                        span class="info-value mono" { @if let Some(bid) = order.bom_snapshot_id { "#" (bid) } @else { "—" } }
                    }
                    div class="info-item" {
                        span class="info-label" { "工艺路线" }
                        span class="info-value mono" { @if let Some(rid) = order.routing_id { "#" (rid) } @else { "—" } }
                    }
                    div class="info-item" { span class="info-label" { "工序数" } span class="info-value mono" { (routing_count) } }
                    div class="info-item" { span class="info-label" { "物料模式" } span class="info-value" { "—" } }
                    div class="info-item" { span class="info-label" { "超额容差" } span class="info-value" { "—" } }
                    div class="info-item" { span class="info-label" { "创建时间" } span class="info-value mono" { (fmt_dt(order.created_at)) } }
                }
            }
        }
        @if !order.remark.is_empty() {
            div class="info-section" {
                div class="info-section-title" { "备注" }
                p class="remark-text" { (order.remark.as_str()) }
            }
        }
    }
}

fn tab_routing(routings: &[WorkOrderRouting]) -> Markup {
    html! {
        // 工序定义表（执行进度已迁移至 batch_routing_progress，由批次维度页面展示）
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "序号" }
                            th { "工序名称" }
                            th { "工作中心" }
                            th class="num-right" { "计划量" }
                            th class="num-right" { "标准工时" }
                            th { "委外" }
                            th { "标记" }
                        }
                    }
                    tbody {
                        @for r in routings {
                            tr {
                                td class="mono" { (r.step_no) }
                                td { strong { (r.process_name.as_str()) } }
                                td class="mono" {
                                    @if let Some(wc) = r.work_center_id { "#" (wc) } @else { "—" }
                                }
                                td class="mono num-right" { (crate::utils::fmt_qty(r.planned_qty)) }
                                td class="mono num-right" {
                                    @if let Some(t) = r.standard_time { (crate::utils::fmt_qty(t)) } @else { "—" }
                                }
                                td {
                                    @if r.is_outsourced { span class="tag-chip" { "委外" } } @else { "—" }
                                }
                                td {
                                    @if r.is_inspection_point {
                                        span class="tag-chip" { "报检" }
                                    } @else { "—" }
                                }
                            }
                        }
                        @if routings.is_empty() {
                            tr { td colspan="7" class="empty-row" { "暂无工序明细（工单未下达或无工艺路线）" } }
                        }
                    }
                }
            }
        }
    }
}

fn tab_batches(batches: &[ProductionBatch], routings: &[WorkOrderRouting], order: &WorkOrder) -> Markup {
    // 计算可拆批余量
    let existing_qty: rust_decimal::Decimal =
        batches.iter().map(|b| b.batch_qty).sum();
    let remaining = order.planned_qty - existing_qty;
    let can_split = matches!(order.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction);

    html! {
        // 操作栏
        @if can_split {
            div class="filter-bar" style="justify-content:flex-end;margin-bottom:var(--space-3)" {
                button class="btn btn-primary" type="button"
                    _="on click add .is-open to #split-dialog" {
                    (icon::plus_icon("w-4 h-4"))
                    "新增批次"
                }
            }
        }

        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "批次号" }
                            th { "流转卡号" }
                            th class="num-right" { "计划量" }
                            th class="num-right" { "完成量" }
                            th class="num-right" { "报废量" }
                            th { "当前工序" }
                            th { "状态" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for b in batches {
                            tr {
                                td class="mono" { (b.batch_no.as_str()) }
                                td class="mono" { (b.card_sn.as_str()) }
                                td class="mono num-right" { (crate::utils::fmt_qty(b.batch_qty)) }
                                td class="mono num-right" style="color:var(--success)" { (crate::utils::fmt_qty(b.completed_qty)) }
                                td class="mono num-right" style="color:var(--danger)" { (crate::utils::fmt_qty(b.scrap_qty)) }
                                td {
                                    @if b.current_step == 0 {
                                        span style="color:var(--muted)" { "未开始" }
                                    } @else {
                                        @let total = routings.len();
                                        @let sname = routings.iter()
                                            .find(|r| r.step_no == b.current_step)
                                            .map(|r| r.process_name.as_str())
                                            .unwrap_or("—");
                                        span { (b.current_step) "/" (total) " " (sname) }
                                    }
                                }
                                td { (batch_status_pill(b.status)) }
                                td { a class="link-cell" href=(format!("/admin/mes/batches/{}", b.id)) { "查看" } }
                            }
                        }
                        @if batches.is_empty() {
                            tr { td colspan="8" class="empty-row" { @if can_split { "暂无生产批次，请点击「新增批次」创建" } @else { "暂无生产批次（工单未下达或无工艺路线）" } } }
                        }
                    }
                }
            }
        }

        // 拆批对话框
        @if can_split {
            div class="modal-overlay" id="split-dialog" {
                div class="modal modal-sm" {
                    div class="modal-head" {
                        h2 { "新增生产批次" }
                    }
                    form {
                        div class="modal-body" {
                            p class="modal-desc" {
                                "工单计划量 " strong { (crate::utils::fmt_qty(order.planned_qty)) }
                                "，已分批 " strong { (crate::utils::fmt_qty(existing_qty)) }
                                @if remaining > rust_decimal::Decimal::ZERO {
                                    "，可新增 " strong style="color:var(--success)" { (crate::utils::fmt_qty(remaining)) }
                                } @else {
                                    "（已全部分配，可按容差新增）"
                                }
                            }
                            div class="form-field" {
                                label { "新增批次数量" }
                                input class="form-input" type="number" step="0.01" name="split_qty"
                                    placeholder="输入数量"
                                    required;
                            }
                            // 工艺路线预览
                            @if !routings.is_empty() {
                                div class="form-field" {
                                    label { "工艺路线（该批次将依次经过以下工序）" }
                                    div style="display:flex;flex-wrap:wrap;gap:6px;padding:8px;background:var(--surface);border-radius:var(--radius-sm);border:1px solid var(--border)" {
                                        @for (i, r) in routings.iter().enumerate() {
                                            @if i > 0 {
                                                span style="color:var(--text-muted);display:flex;align-items:center" { "\u{2192}" }
                                            }
                                            span style="display:inline-flex;align-items:center;gap:4px;padding:2px 8px;background:var(--surface-2);border-radius:var(--radius-sm);font-size:var(--text-xs)" {
                                                span style="font-weight:600;color:var(--primary)" { (r.step_no) }
                                                (r.process_name.as_str())
                                                @if r.is_inspection_point {
                                                    span class="tag-chip" style="font-size:10px;padding:1px 4px" { "检" }
                                                }
                                                @if r.is_outsourced {
                                                    span class="tag-chip" style="font-size:10px;padding:1px 4px" { "外" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div class="modal-foot" {
                            button class="btn btn-default" type="button"
                                _="on click remove .is-open from #split-dialog" {
                                "取消"
                            }
                            button class="btn btn-primary" type="submit"
                                hx-post=(OrderSplitPath { order_id: order.id }.to_string())
                                hx-disabled-elt="this" {
                                "确认新增"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn tab_reports(reports: &[ReportListItem]) -> Markup {
    html! {
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "报工时间" }
                            th { "工序" }
                            th { "报工单号" }
                            th class="num-right" { "完成量" }
                            th class="num-right" { "报废量" }
                            th { "报工人" }
                            th { "班次" }
                        }
                    }
                    tbody {
                        @for r in reports {
                            tr {
                                td class="mono" { (fmt_dt(r.created_at)) }
                                td { (r.process_name.as_str()) }
                                td { a class="link-cell mono" href=(format!("/admin/mes/reports/{}", r.id)) { (r.doc_number.as_str()) } }
                                td class="mono num-right" { (crate::utils::fmt_qty(r.completed_qty)) }
                                td class="mono num-right" { (crate::utils::fmt_qty(r.defect_qty)) }
                                td { (r.worker_name.as_deref().unwrap_or("—")) }
                                td { (shift_label(r.shift)) }
                            }
                        }
                        @if reports.is_empty() {
                            tr { td colspan="7" class="empty-row" { "暂无报工记录" } }
                        }
                    }
                }
            }
        }
    }
}

fn tab_log(logs: &[AuditLog]) -> Markup {
    html! {
        div class="info-section" {
            div class="audit-timeline" {
                @for log in logs {
                    div class="audit-item" {
                        div class="audit-dot" {}
                        div class="audit-content" {
                            div class="audit-title" { (audit_action_label(log.action)) }
                            div class="audit-meta" {
                                span { (fmt_dt(log.created_at)) }
                                span class="sep" { "·" }
                                span { "操作人 #" (log.operator_id) }
                            }
                            @if let Some(changes) = log.changes.as_ref() {
                                div class="audit-desc" { (changes) }
                            }
                        }
                    }
                }
                @if logs.is_empty() {
                    div class="empty-row" { "暂无操作日志" }
                }
            }
        }
    }
}
