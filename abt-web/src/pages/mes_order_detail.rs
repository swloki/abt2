use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::enums::WorkOrderStatus;
use abt_core::mes::work_order::WorkOrderService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{
    OrderCancelPath, OrderClosePath, OrderDetailPath, OrderListPath, OrderReleasePath,
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
    let svc = state.work_order_service();

    let order = svc
        .find_by_id(&service_ctx, &mut conn, path.id)
        .await?;

    let product_name = svc.get_product_name(&mut conn, order.product_id).await?.unwrap_or_default();

    let content = order_detail_page(&order, &product_name);
    let page_html = admin_page(
        is_htmx,
        "工单详情",
        &claims,
        "production",
        &format!("/admin/mes/orders/{}", path.id),
        "生产管理",
        Some(OrderListPath::PATH),
        content, &nav_filter,    );
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

    // 幂等检查：已处于目标状态时直接重定向，防止双重提交报错
    if order.status == WorkOrderStatus::Released {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.release(&service_ctx, &mut conn, path.order_id, order.version).await?;

    let redirect = OrderDetailPath {
        id: path.order_id,
    }
    .to_string();
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

    // 幂等检查：已处于目标状态时直接重定向
    if order.status == WorkOrderStatus::Closed {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.close(&service_ctx, &mut conn, path.order_id, order.version).await?;

    let redirect = OrderDetailPath {
        id: path.order_id,
    }
    .to_string();
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

    // 幂等检查：已处于目标状态时直接重定向
    if order.status == WorkOrderStatus::Cancelled {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.cancel(&service_ctx, &mut conn, path.order_id, order.version).await?;

    let redirect = OrderDetailPath {
        id: path.order_id,
    }
    .to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn order_detail_page(order: &abt_core::mes::work_order::WorkOrder, product_name: &str) -> Markup {
    let (status_label, status_bg, status_color) = wo_status_label(&order.status);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(OrderListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回工单列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (order.doc_number) }
                        (status_pill(status_label, status_bg, status_color))
                    }
                }
                div class="page-actions" {
                    @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned) {
                        button class="btn btn-primary"
                            hx-post=(OrderReleasePath { order_id: order.id }.to_string())
                            hx-confirm="确认下达此工单？下达后将开始生产。"
                            hx-disabled-elt="this" {
                            (icon::rocket_icon("w-4 h-4"))
                            "下达工单"
                        }
                    }
                    @if matches!(order.status, WorkOrderStatus::Released) {
                        button class="btn btn-default"
                            hx-post=(OrderClosePath { order_id: order.id }.to_string())
                            hx-confirm="确认关闭此工单？"
                            hx-disabled-elt="this" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "关闭工单"
                        }
                    }
                    @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned | WorkOrderStatus::Released) {
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

            // ── Order Info ──
            div class="info-card" {
                div class="info-card-title" { "工单信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "工单编号" }
                        span class="info-value mono" { (order.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "产品" }
                        span class="info-value" { (product_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "计划数量" }
                        span class="info-value mono" { (crate::utils::fmt_qty(order.planned_qty)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "计划开始日期" }
                        span class="info-value mono" { (order.scheduled_start) }
                    }
                    div class="info-item" {
                        span class="info-label" { "计划结束日期" }
                        span class="info-value mono" { (order.scheduled_end) }
                    }
                    div class="info-item" {
                        span class="info-label" { "状态" }
                        span class="info-value" { (status_pill(status_label, status_bg, status_color)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "版本" }
                        span class="info-value mono" { (order.version) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value mono" style="font-size:12px" {
                            (order.created_at.format("%Y-%m-%d %H:%M"))
                        }
                    }
                }
            }

            // ── Remark ──
            @if !order.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-4)" {
                    div class="info-card-title" { "备注" }
                    p style="color:var(--muted);font-size:var(--text-sm)" { (order.remark.as_str()) }
                }
            }
        }
    }
}
