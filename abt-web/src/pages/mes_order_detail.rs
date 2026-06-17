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
    // 完工率
    let completion_pct = if order.planned_qty > rust_decimal::Decimal::ZERO {
        ((order.completed_qty / order.planned_qty) * rust_decimal::Decimal::ONE_HUNDRED)
            .min(rust_decimal::Decimal::ONE_HUNDRED)
    } else {
        rust_decimal::Decimal::ZERO
    };

    // 是否有入库记录
    let has_receipts = order.completed_qty > rust_decimal::Decimal::ZERO;

    let content = order_detail_page(
        &order, &product_name, &routings, &batches, &reports, &audit_logs,
        completion_pct, has_receipts,
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

#[allow(clippy::too_many_arguments)]
fn order_detail_page(
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
    batches: &[ProductionBatch],
    reports: &[ReportListItem],
    audit_logs: &[AuditLog],
    completion_pct: rust_decimal::Decimal,
    has_receipts: bool,
) -> Markup {
    let (status_label, status_bg, status_color) = wo_status_label(&order.status);
    let routing_tab_label = format!("工序明细 {}", routings.len());

    html! {
        div {
            // 返回
            a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", OrderListPath::PATH)) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回工单列表"
            }

            // Detail Header
            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div class="flex items-center justify-between" {
                    div class="text-[24px] font-bold text-fg flex items-center gap-[14px] font-mono tabular-nums" {
                        span { (order.doc_number) }
                        (status_pill(status_label, status_bg, status_color))
                    }
                    div class="flex gap-3" {
                        @if matches!(order.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button" _="on click add .is-open to #unrelease-dialog" {
                                "反下达"
                            }
                            @if completion_pct >= rust_decimal::Decimal::new(95, 2) {
                                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                                    hx-post=(OrderClosePath { order_id: order.id }.to_string())
                                    hx-confirm="确认关闭此工单？所有批次必须已完工或已取消。"
                                    hx-disabled-elt="this" {
                                    (icon::check_circle_icon("w-4 h-4"))
                                    "关闭工单"
                                }
                            } @else {
                                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" disabled
                                    title=(format!("完工率 {}%，需 ≥ 95% 才能关闭", completion_pct.round_dp(1))) {
                                    (icon::check_circle_icon("w-4 h-4"))
                                    "关闭工单（完工不足）"
                                }
                            }
                        }
                        @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned) {
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                                hx-post=(OrderReleasePath { order_id: order.id }.to_string())
                                hx-confirm="确认下达此工单？下达后将开始生产。"
                                hx-disabled-elt="this" {
                                (icon::rocket_icon("w-4 h-4"))
                                "下达工单"
                            }
                        }
                        @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned | WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
                            @if has_receipts {
                                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90" disabled
                                    title="存在已完工入库记录，无法取消" {
                                    (icon::x_icon("w-4 h-4"))
                                    "取消（有入库记录）"
                                }
                            } @else {
                                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                                    hx-post=(OrderCancelPath { order_id: order.id }.to_string())
                                    hx-confirm="确认取消此工单？取消后不可恢复。"
                                    hx-disabled-elt="this" {
                                    (icon::x_icon("w-4 h-4"))
                                    "取消工单"
                                }
                            }
                        }
                    }
                }

                // 副标题行
                div class="flex items-center flex-wrap gap-2 text-text-muted text-sm" {
                    span { (product_name) }
                    span class="sep" { "|" }
                    span class="font-mono tabular-nums" { (crate::utils::fmt_qty(order.planned_qty)) " 件" }
                    @if order.completed_qty > rust_decimal::Decimal::ZERO {
                        span class="sep" { "|" }
                        span class="font-mono tabular-nums" style="color:var(--success)" { "完成 " (crate::utils::fmt_qty(order.completed_qty)) }
                    }
                    @if order.scrap_qty > rust_decimal::Decimal::ZERO {
                        span class="sep" { "|" }
                        span class="font-mono tabular-nums" style="color:var(--danger)" { "报废 " (crate::utils::fmt_qty(order.scrap_qty)) }
                    }
                    span class="sep" { "|" }
                    span { "—" }
                    @if let Some(so) = order.source_so_doc.as_ref() {
                        span class="sep" { "|" }
                        span { "销售订单: " (so) }
                        @if let Some(c) = order.source_customer.as_ref() {
                            span class="text-muted" { " (" (c) ")" }
                        }
                    }
                    @if let Some(pdoc) = order.source_plan_doc.as_ref() {
                        span class="sep" { "|" }
                        span { "生产计划: " }
                        @if let Some(pid) = order.source_plan_id {
                            a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/plans/{pid}")) { (pdoc) }
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

            (tab_panel("info", true, tab_info(order, product_name, routings.len(), completion_pct)))
            (tab_panel("routing", false, tab_routing(routings)))
            (tab_panel("batches", false, tab_batches(batches, routings, order)))
            (tab_panel("reports", false, tab_reports(reports)))
            (tab_panel("log", false, tab_log(audit_logs)))

            // 反下达对话框
            @if matches!(order.status, WorkOrderStatus::Released) {
                div class="fixed z-[1000] grid place-items-center opacity-0" id="unrelease-dialog" {
                    div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-sm" {
                        div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                            h2 { "确认反下达？" }
                        }
                        div class="overflow-y-auto flex-1 min-h-0 p-6" {
                            p class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-desc" {
                                "反下达将回退工单到 "
                                strong { "草稿" }
                                " 状态，同时取消领料单、释放库存预留、软删除生产批次（若有报工记录则无法反下达）。此操作不可撤销。"
                            }
                        }
                        div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button" _="on click remove .is-open from #unrelease-dialog" {
                                "取消"
                            }
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
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
fn tab_info(order: &WorkOrder, product_name: &str, routing_count: usize, completion_pct: rust_decimal::Decimal) -> Markup {
    let (sl, sb, sc) = wo_status_label(&order.status);
    html! {
        div class="grid gap-5 bg-bg border border-border-soft rounded-lg p-6" {
            div class="flex flex-col gap-4" {
                div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "基础信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "工单编号" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (order.doc_number) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "产品" } span class="text-sm text-fg font-medium" { (product_name) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "计划数量" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (crate::utils::fmt_qty(order.planned_qty)) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "状态" } span class="text-sm text-fg font-medium" { (status_pill(sl, sb, sc)) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "版本号" } span class="text-sm text-fg font-medium font-mono tabular-nums" { "v"(order.version) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "计划开始" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (order.scheduled_start) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "计划结束" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (order.scheduled_end) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "创建人" } span class="text-sm text-fg font-medium font-mono tabular-nums" { "#" (order.operator_id) } }
                }
            }
            div class="flex flex-col gap-4" {
                div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "生产配置" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "BOM 快照" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { @if let Some(bid) = order.bom_snapshot_id { "#" (bid) } @else { "—" } }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "工艺路线" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { @if let Some(rid) = order.routing_id { "#" (rid) } @else { "—" } }
                    }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "工序数" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (routing_count) } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "物料模式" } span class="text-sm text-fg font-medium" { "—" } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "超额容差" } span class="text-sm text-fg font-medium" { "—" } }
                    div class="flex flex-col gap-1" { span class="text-xs text-text-muted font-medium" { "创建时间" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (fmt_dt(order.created_at)) } }
                }
            }
        }
        // 生产进度
        div class="bg-bg border border-border-soft rounded-lg p-6" {
            div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "生产进度" }
            div class="py-3" {
                div class="flex gap-[24px]" {
                    span class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "计划" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (crate::utils::fmt_qty(order.planned_qty)) }
                    }
                    span class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "已完工" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (crate::utils::fmt_qty(order.completed_qty)) }
                    }
                    span class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "完工率" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (completion_pct.round_dp(1)) "%" }
                    }
                }
                div class="h-[8px] bg-[#f5f5f5] overflow-hidden" {
                    div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden-fill"
                        style=(format!("width: {}%", completion_pct.round_dp(1)))
                    {}
                }
            }
        }
        @if !order.remark.is_empty() {
            div class="bg-bg border border-border-soft rounded-lg p-6" {
                div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "备注" }
                p class="text-sm text-text-muted" { (order.remark.as_str()) }
            }
        }
    }
}

fn tab_routing(routings: &[WorkOrderRouting]) -> Markup {
    html! {
        // 工序定义表（执行进度已迁移至 batch_routing_progress，由批次维度页面展示）
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "序号" }
                            th { "工序名称" }
                            th { "工作中心" }
                            th class="text-right text-[13px]" { "计划量" }
                            th class="text-right text-[13px]" { "标准工时" }
                            th class="text-right text-[13px]" { "标准成本" }
                            th class="text-right text-[13px]" { "计件单价" }
                            th { "委外" }
                            th { "标记" }
                        }
                    }
                    tbody {
                        @for r in routings {
                            tr {
                                td class="font-mono tabular-nums" { (r.step_no) }
                                td { strong { (r.process_name.as_str()) } }
                                td class="font-mono tabular-nums" {
                                    @if let Some(wc) = r.work_center_id { "#" (wc) } @else { "—" }
                                }
                                td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.planned_qty)) }
                                td class="font-mono tabular-nums text-right text-[13px]" {
                                    @if let Some(t) = r.standard_time { (crate::utils::fmt_qty(t)) } @else { "—" }
                                }
                                td class="font-mono tabular-nums text-right text-[13px]" {
                                    @if let Some(c) = r.standard_cost { "¥" (crate::utils::fmt_qty(c)) } @else { "—" }
                                }
                                td class="font-mono tabular-nums text-right text-[13px]" {
                                    @if let Some(p) = r.unit_price { "¥" (crate::utils::fmt_qty(p)) } @else { "—" }
                                }
                                td {
                                    @if r.is_outsourced { span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium" { "委外" } } @else { "—" }
                                }
                                td {
                                    @if r.is_inspection_point {
                                        span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium" { "报检" }
                                    } @else { "—" }
                                }
                            }
                        }
                        @if routings.is_empty() {
                            tr { td colspan="9" class="text-center text-text-muted text-sm" { "暂无工序明细（工单未下达或无工艺路线）" } }
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
            div class="flex items-center gap-3 mb-5 flex-wrap" style="justify-content:flex-end;margin-bottom:var(--space-3)" {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="button"
                    _="on click add .is-open to #split-dialog" {
                    (icon::plus_icon("w-4 h-4"))
                    "新增批次"
                }
            }
        }

        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "批次号" }
                            th { "流转卡号" }
                            th class="text-right text-[13px]" { "计划量" }
                            th class="text-right text-[13px]" { "完成量" }
                            th class="text-right text-[13px]" { "报废量" }
                            th { "当前工序" }
                            th { "状态" }
                            th class="text-right" { "操作" }
                        }
                    }
                    tbody {
                        @for b in batches {
                            tr {
                                td class="font-mono tabular-nums" { (b.batch_no.as_str()) }
                                td class="font-mono tabular-nums" { (b.card_sn.as_str()) }
                                td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(b.batch_qty)) }
                                td class="font-mono tabular-nums text-right text-[13px]" style="color:var(--success)" { (crate::utils::fmt_qty(b.completed_qty)) }
                                td class="font-mono tabular-nums text-right text-[13px]" style="color:var(--danger)" { (crate::utils::fmt_qty(b.scrap_qty)) }
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
                                td { a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/batches/{}", b.id)) { "查看" } }
                            }
                        }
                        @if batches.is_empty() {
                            tr { td colspan="8" class="text-center text-text-muted text-sm" { @if can_split { "暂无生产批次，请点击「新增批次」创建" } @else { "暂无生产批次（工单未下达或无工艺路线）" } } }
                        }
                    }
                }
            }
        }

        // 拆批对话框
        @if can_split {
            div class="fixed z-[1000] grid place-items-center opacity-0" id="split-dialog" {
                div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-sm" {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                        h2 { "新增生产批次" }
                    }
                    form {
                        div class="overflow-y-auto flex-1 min-h-0 p-6" {
                            p class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-desc" {
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
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.01" name="split_qty"
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
                                                    span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium" style="font-size:10px;padding:1px 4px" { "检" }
                                                }
                                                @if r.is_outsourced {
                                                    span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium" style="font-size:10px;padding:1px 4px" { "外" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button"
                                _="on click remove .is-open from #split-dialog" {
                                "取消"
                            }
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit"
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
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "报工时间" }
                            th { "工序" }
                            th { "报工单号" }
                            th class="text-right text-[13px]" { "完成量" }
                            th class="text-right text-[13px]" { "报废量" }
                            th { "报工人" }
                            th { "班次" }
                        }
                    }
                    tbody {
                        @for r in reports {
                            tr {
                                td class="font-mono tabular-nums" { (fmt_dt(r.created_at)) }
                                td { (r.process_name.as_str()) }
                                td { a class="text-accent font-medium cursor-pointer font-mono tabular-nums" href=(format!("/admin/mes/reports/{}", r.id)) { (r.doc_number.as_str()) } }
                                td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.completed_qty)) }
                                td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.defect_qty)) }
                                td { (r.worker_name.as_deref().unwrap_or("—")) }
                                td { (shift_label(r.shift)) }
                            }
                        }
                        @if reports.is_empty() {
                            tr { td colspan="7" class="text-center text-text-muted text-sm" { "暂无报工记录" } }
                        }
                    }
                }
            }
        }
    }
}

fn tab_log(logs: &[AuditLog]) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-6" {
            div class="relative pl-6 before:content-[''] before:absolute before:left-[7px] before:top-1 before:bottom-1 before:w-0.5 before:bg-border-soft" {
                @for log in logs {
                    div class="relative pb-5 last:pb-0" {
                        div class="absolute w-[14px] h-[14px] rounded-full bg-accent" {}
                        div class="pl-0" {
                            div class="font-semibold text-sm text-fg" { (audit_action_label(log.action)) }
                            div class="flex gap-2 text-[12px] text-text-muted items-center" {
                                span { (fmt_dt(log.created_at)) }
                                span class="sep" { "·" }
                                span { "操作人 #" (log.operator_id) }
                            }
                            @if let Some(changes) = log.changes.as_ref() {
                                div class="text-[12px] text-text-muted" { (changes) }
                            }
                        }
                    }
                }
                @if logs.is_empty() {
                    div class="text-center text-text-muted text-sm" { "暂无操作日志" }
                }
            }
        }
    }
}
