use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};

use crate::state::AppState;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::sales_return::SalesReturnService;
use abt_core::sales::sales_return::model::*;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PgExecutor;
use abt_core::shared::types::ServiceContext;

use crate::components::icon;
use crate::utils::fmt_qty;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::OrderDetailPath;
use crate::routes::sales_return::*;
use crate::routes::shipping::ShippingDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: ReturnStatus) -> (&'static str, &'static str) {
    match s {
        ReturnStatus::Draft => ("草稿", "status-draft"),
        ReturnStatus::Confirmed => ("已确认", "status-confirmed"),
        ReturnStatus::Received => ("已收货", "status-progress"),
        ReturnStatus::Inspecting => ("质检中", "status-inspecting"),
        ReturnStatus::Completed => ("已完成", "status-completed"),
        ReturnStatus::Cancelled => ("已取消", "status-cancelled"),
        ReturnStatus::Rejected => ("已驳回", "status-rejected"),
    }
}

fn disposition_label(d: ReturnDisposition) -> &'static str {
    match d {
        ReturnDisposition::Restock => "退回库存",
        ReturnDisposition::Scrap => "报废",
        ReturnDisposition::Rework => "返工",
    }
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_return_detail(
    path: ReturnDetailPath,
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

    // Fetch return header
    let ret = state
        .sales_return_service()
        .find_by_id(&service_ctx, &mut conn, path.id)
        .await?;

    // Fetch return items
    let items = state
        .sales_return_service()
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

    // Resolve customer name
    let customer_name = state
        .customer_service()
        .get(&service_ctx, &mut conn, ret.customer_id)
        .await
        .map(|c| c.name)
        .unwrap_or_else(|_| "未知客户".into());

    // Resolve order number
    let order_number = state
        .sales_order_service()
        .find_by_id(&service_ctx, &mut conn, ret.order_id)
        .await
        .map(|o| o.doc_number)
        .unwrap_or_else(|_| "—".into());

    // Resolve shipping number
    let shipping_number = state
        .shipping_service()
        .find_by_id(&service_ctx, &mut conn, ret.shipping_request_id)
        .await
        .map(|s| s.doc_number)
        .unwrap_or_else(|_| "—".into());

    // Resolve operator name
    let operator_name = state
        .user_service()
        .get_user(&service_ctx, &mut conn, ret.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    // Resolve product details
    let product_details = resolve_product_details(&state, &service_ctx, &mut conn, &items).await;

    let content = return_detail_page(
        &ret,
        &items,
        &customer_name,
        &order_number,
        &shipping_number,
        &operator_name,
        &product_details,
    );
    let page_html = admin_page(
        is_htmx,
        "退货详情",
        &claims,
        "sales",
        &format!("{}/{}", ReturnListPath::PATH, path.id),
        "销售管理",
        Some("退货详情"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_return(
    path: ConfirmReturnPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    state
        .sales_return_service()
        .approve(&service_ctx, &mut conn, path.id)
        .await?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn receive_return(
    path: ReceiveReturnPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    state
        .sales_return_service()
        .receive(&service_ctx, &mut conn, path.id)
        .await?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn inspect_return(
    path: InspectReturnPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    state
        .sales_return_service()
        .inspect(&service_ctx, &mut conn, path.id)
        .await?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn complete_return(
    path: CompleteReturnPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    state
        .sales_return_service()
        .complete(&service_ctx, &mut conn, path.id)
        .await?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn reject_return(
    path: RejectReturnPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    state
        .sales_return_service()
        .reject(&service_ctx, &mut conn, path.id)
        .await?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Product Detail Resolution ──

struct ProductDetail {
    code: String,
    name: String,
    unit: String,
}

async fn resolve_product_details(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    items: &[SalesReturnItem],
) -> HashMap<i64, ProductDetail> {
    let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let products = state
        .product_service()
        .get_by_ids(ctx, db, ids)
        .await
        .unwrap_or_default();
    products
        .into_iter()
        .map(|p| {
            (
                p.product_id,
                ProductDetail {
                    code: p.product_code,
                    name: p.pdt_name,
                    unit: p.unit,
                },
            )
        })
        .collect()
}

// ── Workflow Steps ──

fn workflow_steps(current: ReturnStatus) -> Markup {
    let steps: &[(&str, ReturnStatus)] = &[
        ("草稿", ReturnStatus::Draft),
        ("已确认", ReturnStatus::Confirmed),
        ("已收货", ReturnStatus::Received),
        ("质检中", ReturnStatus::Inspecting),
        ("已完成", ReturnStatus::Completed),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == ReturnStatus::Cancelled;
    let is_rejected = current == ReturnStatus::Rejected;
    let terminal = is_cancelled || is_rejected;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=({
                            if i <= current_idx && !terminal {
                                "wf-line current"
                            } else {
                                "wf-line"
                            }
                        }) {}
                }
                @let step_class = if terminal {
                    "wf-step"
                } else if i < current_idx {
                    "wf-step completed"
                } else if i == current_idx {
                    "wf-step current"
                } else {
                    "wf-step"
                };
                div class=(step_class) {
                    div class="wf-dot" {}
                    (label)
                }
            }
            @if is_cancelled {
                div class="wf-step cancelled danger" {
                    div class="wf-dot" {}
                    "已取消"
                }
            }
            @if is_rejected {
                div class="wf-step cancelled danger" {
                    div class="wf-dot" {}
                    "已驳回"
                }
            }
        }
    }
}

// ── Components ──

fn return_detail_page(
    r: &SalesReturn,
    items: &[SalesReturnItem],
    customer_name: &str,
    order_number: &str,
    shipping_number: &str,
    operator_name: &str,
    product_details: &HashMap<i64, ProductDetail>,
) -> Markup {
    let (status_text, status_class) = status_label(r.status);
    let shipping_detail = ShippingDetailPath {
        id: r.shipping_request_id,
    };
    let order_detail = OrderDetailPath { id: r.order_id };

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(ReturnListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回退货列表"
            }
            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (r.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                    div class="detail-source" {
                        "来源发货："
                        a href=(shipping_detail.to_string()) { (shipping_number) }
                        "　来源订单："
                        a href=(order_detail.to_string()) {
                            (order_number)
                        }
                    }
                }
                div class="page-actions" {
                    a class="btn btn-default" href=(ReturnListPath::PATH) { "返回列表" }
                    @if r.status == ReturnStatus::Draft {
                        button
                            class="btn btn-primary"
                            hx-post=(ConfirmReturnPath { id: r.id }.to_string())
                            hx-confirm="确认审核此退货单？"
                        { "确认退货" }
                    }
                    @if r.status == ReturnStatus::Confirmed {
                        button
                            class="btn btn-primary"
                            hx-post=(ReceiveReturnPath { id: r.id }.to_string())
                            hx-confirm="确认已收到退货？"
                        { "确认收货" }
                    }
                    @if r.status == ReturnStatus::Received {
                        button
                            class="btn btn-primary"
                            hx-post=(InspectReturnPath { id: r.id }.to_string())
                            hx-confirm="确认开始质检？"
                        { "开始质检" }
                    }
                    @if r.status == ReturnStatus::Inspecting {
                        button
                            class="btn btn-success"
                            hx-post=(CompleteReturnPath { id: r.id }.to_string())
                            hx-confirm="确认完成退货？"
                        { "完成退货" }
                        button
                            class="btn btn-danger"
                            hx-post=(RejectReturnPath { id: r.id }.to_string())
                            hx-confirm="确认驳回此退货？"
                        { "驳回" }
                    }
                }
            }
            // ── Workflow Steps ──
            (workflow_steps(r.status))
            // ── Return Info ──
            div class="info-card" {
                div class="info-card-title" { "退货信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户名称" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "退货原因" }
                        span class="info-value" { (r.return_reason.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { (operator_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value" { (r.created_at.format("%Y-%m-%d %H:%M")) }
                    }
                }
            }
            // ── Items Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "单位" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "退货数量" }
                                th class="num-right" { "退货金额" }
                                th { "处理方式" }
                            }
                        }
                        tbody {
                            @for (i, item) in items.iter().enumerate() {
                                (item_row(i, item, product_details))
                            }
                            @if items.is_empty() {
                                tr {
                                    td  colspan="8" class="td-empty"
                                    { "暂无明细" }
                                }
                            }
                        }
                    }
                }
                div class="amount-summary" {
                    div class="amount-row" {
                        span { "退货总额" }
                        span class="mono stat-value-lg" {
                            "¥ "
                            (format!("{:.2}", r.total_amount))
                        }
                    }
                }
            }
            // ── Remarks ──
            @if !r.remark.is_empty() {
                div class="info-card mt-6" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (r.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(index: usize, item: &SalesReturnItem, details: &HashMap<i64, ProductDetail>) -> Markup {
    let detail = details.get(&item.product_id);
    let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
    let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
    let unit = detail.map(|d| d.unit.as_str()).unwrap_or("—");

    html! {
        tr {
            td class="mono" { (index + 1) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (unit) }
            td class="num-right mono" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (fmt_qty(item.returned_qty)) }
            td class="num-right mono" { (format!("{:.2}", item.amount)) }
            td { (disposition_label(item.disposition)) }
        }
    }
}
