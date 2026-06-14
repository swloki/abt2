use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::enums::{BatchStatus, RoutingStatus, ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{ProductionBatch, ProductionBatchService, WorkOrderRouting};
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
    OrderUnreleasePath,
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

fn routing_status_pill(s: RoutingStatus) -> Markup {
    let (l, bg, c) = match s {
        RoutingStatus::Pending => ("待生产", "rgba(0,0,0,0.04)", "var(--muted)"),
        RoutingStatus::InProgress => ("进行中", "rgba(22,119,255,0.08)", "var(--accent)"),
        RoutingStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        RoutingStatus::Skipped => ("已跳过", "rgba(0,0,0,0.04)", "var(--muted)"),
    };
    status_pill(l, bg, c)
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
    let total = routings.len() as i32;
    let done = routings
        .iter()
        .filter(|r| matches!(r.status, RoutingStatus::Completed))
        .count() as i32;
    let pct = if total > 0 { done * 100 / total } else { 0 };
    let fill_cls = if pct < 34 {
        "low"
    } else if pct < 67 {
        "mid"
    } else {
        "high"
    };
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
                // 进度条（有工序时）
                @if total > 0 {
                    div class="wo-progress" {
                        div class="wo-progress-label" {
                            span class="label-text" { "生产进度" }
                            span class="label-value" { (format!("{pct}% ({done}/{total} 工序)")) }
                        }
                        div class="wo-progress-track" {
                            div class=(format!("wo-progress-fill {fill_cls}")) style=(format!("width:{pct}%")) {}
                        }
                    }
                }
            }

            // Tabs
            (detail_tabs("info", &[
                ("info", "工单信息"),
                ("routing", &routing_tab_label),
                ("docs", "关联单据"),
                ("log", "操作日志"),
            ]))

            (tab_panel("info", true, tab_info(order, product_name, routings.len())))
            (tab_panel("routing", false, tab_routing(routings)))
            (tab_panel("docs", false, tab_docs(batches, reports, routings.len())))
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
                                " 状态，同时取消领料单、释放库存预留、删除生产批次和工序记录。此操作不可撤销。"
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
        @if !routings.is_empty() {
            div class="flow-flow-wrap" {
                div class="flow-flow-wrap-title" { "工序进度可视化" }
                div class="routing-flow" {
                    @for (i, r) in routings.iter().enumerate() {
                        @if i > 0 {
                            @let prev_done = matches!(routings[i - 1].status, RoutingStatus::Completed);
                            div class=(if prev_done { "flow-arrow completed" } else { "flow-arrow" }) { "→" }
                        }
                        @let node_cls = match r.status {
                            RoutingStatus::Completed => "flow-node completed",
                            RoutingStatus::InProgress => "flow-node current",
                            _ => "flow-node pending",
                        };
                        div class=(node_cls) {
                            div class="flow-icon" {
                                @if matches!(r.status, RoutingStatus::Completed) { "✓" }
                                @else if matches!(r.status, RoutingStatus::InProgress) { "⚡" }
                                @else { (r.step_no) }
                            }
                            div {
                                div class="flow-title" { (r.step_no) ". " (r.process_name.as_str()) }
                                div class="flow-meta" { "—" }
                            }
                            div class="flow-stats" {
                                div class="flow-stat" {
                                    span class="stat-label" { "完成" }
                                    span class="stat-val" { (crate::utils::fmt_qty(r.completed_qty)) }
                                }
                                div class="flow-stat" {
                                    span class="stat-label" { "报废" }
                                    span class="stat-val text-danger" { (crate::utils::fmt_qty(r.defect_qty)) }
                                }
                            }
                        }
                    }
                }
            }
        }
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "序号" }
                            th { "工序名称" }
                            th { "工作中心" }
                            th class="num-right" { "计划量" }
                            th class="num-right" { "完成量" }
                            th class="num-right" { "报废量" }
                            th { "状态" }
                            th { "标记" }
                        }
                    }
                    tbody {
                        @for r in routings {
                            tr {
                                td class="mono" { (r.step_no) }
                                td { strong { (r.process_name.as_str()) } }
                                td { "—" }
                                td class="mono num-right" { (crate::utils::fmt_qty(r.planned_qty)) }
                                td class="mono num-right" { (crate::utils::fmt_qty(r.completed_qty)) }
                                td class="mono num-right" { (crate::utils::fmt_qty(r.defect_qty)) }
                                td { (routing_status_pill(r.status)) }
                                td {
                                    @if r.is_inspection_point {
                                        span class="tag-chip" { "报检" }
                                    } @else { "—" }
                                }
                            }
                        }
                        @if routings.is_empty() {
                            tr { td colspan="8" class="empty-row" { "暂无工序明细（工单未下达或无工艺路线）" } }
                        }
                    }
                }
            }
        }
    }
}

fn tab_docs(batches: &[ProductionBatch], reports: &[ReportListItem], total_steps: usize) -> Markup {
    html! {
        // ── 子 Tab 栏 (Hyperscript 切换) ──
        div class="detail-tabs" style="margin-bottom:var(--space-5)" {
            button class="detail-tab active doc-sub-tab" type="button"
                _="on click remove .active from <.doc-sub-tab/> then add .active to me then hide <.doc-panel/> then show #doc-panel-batches" {
                "生产批次 " span class="tab-count" { (batches.len()) }
            }
            button class="detail-tab doc-sub-tab" type="button"
                _="on click remove .active from <.doc-sub-tab/> then add .active to me then hide <.doc-panel/> then show #doc-panel-reports" {
                "报工记录 " span class="tab-count" { (reports.len()) }
            }
        }

        // ── 子面板：生产批次 ──
        div class="doc-panel" id="doc-panel-batches" {
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "批次号" }
                                th { "流转卡号" }
                                th { "班组" }
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
                                    td { (b.batch_no.as_str()) }
                                    td class="mono" { (b.card_sn.as_str()) }
                                    td { "—" }
                                    td class="mono num-right" { (crate::utils::fmt_qty(b.batch_qty)) }
                                    td class="mono num-right" { (crate::utils::fmt_qty(b.completed_qty)) }
                                    td class="mono num-right" { (crate::utils::fmt_qty(b.scrap_qty)) }
                                    td {
                                        @if b.current_step == 0 {
                                            span style="color:var(--muted)" { "未开始" }
                                        } @else if total_steps > 0 {
                                            span { "第 " (b.current_step) "/" (total_steps) " 步" }
                                        } @else {
                                            span { "第 " (b.current_step) " 步" }
                                        }
                                    }
                                    td { (batch_status_pill(b.status)) }
                                    td { a class="link-cell" href=(format!("/admin/mes/batches/{}", b.id)) { "查看" } }
                                }
                            }
                            @if batches.is_empty() {
                                tr { td colspan="9" class="empty-row" { "暂无生产批次（工单未下达或无工艺路线）" } }
                            }
                        }
                    }
                }
            }
        }

        // ── 子面板：报工记录 ──
        div class="doc-panel" id="doc-panel-reports" style="display:none" {
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
