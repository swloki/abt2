use std::collections::HashMap;
use rust_decimal::Decimal;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::purchase::order::model::*;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PurchaseOrderStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseOrderStatus::Draft => ("草稿", "status-draft"),
        PurchaseOrderStatus::Confirmed => ("已确认", "status-confirmed"),
        PurchaseOrderStatus::PartiallyReceived => ("部分收货", "status-partial"),
        PurchaseOrderStatus::Received => ("已收货", "status-shipped"),
        PurchaseOrderStatus::Closed => ("已关闭", "status-completed"),
        PurchaseOrderStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_detail(
    path: PODetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let order = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, order.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, order.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let (product_names, product_codes, product_units, product_specs) = {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if product_ids.is_empty() {
            (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new())
        } else {
            let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
            let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
            let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
            let units: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.unit.clone())).collect();
            let specs: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.meta.specification.clone())).collect();
            (names, codes, units, specs)
        }
    };
    let content = po_detail_page(&order, &items, &supplier_name, &operator_name, &product_names, &product_codes, &product_units, &product_specs);
    let page_html = admin_page(
        is_htmx, "订单详情", &claims, "purchase",
        &format!("{}/{}", POListPath::PATH, path.id),
        "采购管理", Some("订单详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn confirm_po(
    path: POConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();

    svc.confirm(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PODetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn cancel_po(
    path: POCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();

    svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PODetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: PurchaseOrderStatus) -> Markup {
    let steps: &[(&str, PurchaseOrderStatus)] = &[
        ("草稿", PurchaseOrderStatus::Draft),
        ("已确认", PurchaseOrderStatus::Confirmed),
        ("部分收货", PurchaseOrderStatus::PartiallyReceived),
        ("已收货", PurchaseOrderStatus::Received),
        ("已关闭", PurchaseOrderStatus::Closed),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == PurchaseOrderStatus::Cancelled;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    @let line_class = if i <= current_idx && !is_cancelled { "wf-line completed" } else { "wf-line" };
                    div class=(line_class) {}
                }
                @let step_class = if is_cancelled {
                    "wf-step"
                } else if i < current_idx {
                    "wf-step completed"
                } else if i == current_idx {
                    "wf-step current"
                } else {
                    "wf-step"
                };
                div class=(step_class) {
                    span class="wf-dot" {}
                    (label)
                }
            }
            @if is_cancelled {
                div class="wf-line" {}
                div class="wf-step" style="color:var(--danger)" {
                    span class="wf-dot" {}
                    "已取消"
                }
            }
        }
    }
}

// ── Components ──

fn po_detail_page(
    order: &PurchaseOrder,
    items: &[PurchaseOrderItem],
    supplier_name: &str,
    operator_name: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
    product_units: &HashMap<i64, String>,
    product_specs: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(order.status);
    let expected_delivery = order.expected_delivery_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let payment_terms = order.payment_terms.as_deref().unwrap_or("—");
    let delivery_address = order.delivery_address.as_deref().unwrap_or("—");
    let received_total: Decimal = items.iter()
        .map(|i| i.received_qty * i.unit_price)
        .sum();
    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(POListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回采购订单列表"
            }
            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (order.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    button class="btn btn-default" {
                        (icon::printer_icon("w-4 h-4"))
                        "打印"
                    }
                    button class="btn btn-default" {
                        (icon::link_icon("w-4 h-4"))
                        "关联报价"
                    }
                    @if order.status == PurchaseOrderStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(POConfirmPath { id: order.id }.to_string())
                            hx-confirm="确认此订单？确认后将通知供应商。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认订单"
                        }
                        button class="btn btn-danger"
                            hx-post=(POCancelPath { id: order.id }.to_string())
                            hx-confirm="确认取消此订单？取消后不可恢复。" {
                            "取消订单"
                        }
                    }
                }
            }
            // ── Workflow Steps ──
            (workflow_steps(order.status))
            // ── Order Info ──
            div class="info-card" {
                div class="info-card-title" { "订单信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "供应商" }
                        span class="info-value" { (supplier_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "订单日期" }
                        span class="info-value mono" { (order.order_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "预计到货" }
                        span class="info-value mono" { (expected_delivery) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款条款" }
                        span class="info-value" { (payment_terms) }
                    }
                    div class="info-item" {
                        span class="info-label" { "交货地址" }
                        span class="info-value" { (delivery_address) }
                    }
                    div class="info-item" {
                        span class="info-label" { "币种" }
                        span class="info-value" { "CNY" }
                    }
                    div class="info-item" {
                        span class="info-label" { "采购员" }
                        span class="info-value" { (operator_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "关联报价" }
                        span class="info-value" { "—" }
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
                                th { "物料编码" }
                                th { "物料名称" }
                                th { "规格" }
                                th { "单位" }
                                th class="num-right" { "数量" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "金额" }
                                th class="num-right" { "已收货" }
                                th class="num-right" { "已检验" }
                                th class="num-right" { "已退货" }
                                th { "预计到货" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names, product_codes, product_units, product_specs))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="12" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
                // ── Amount Summary ──
                div class="amount-summary" {
                    div class="amount-row" {
                        span class="amount-label" { "订单总额" }
                        span class="amount-value accent" { (format!("¥ {:.2}", order.total_amount)) }
                    }
                    div class="amount-row" {
                        span class="amount-label" { "已收货金额" }
                        span class="amount-value" { (format!("¥ {:.2}", received_total)) }
                    }
                }
            }
            // ── Remarks ──
            @if !order.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-6)" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (order.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &PurchaseOrderItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
    units: &HashMap<i64, String>,
    specs: &HashMap<i64, String>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let unit = units.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let spec = specs.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let expected_delivery = item.expected_delivery_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    html! {
        tr {
            td class="mono" { (item.line_no) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (spec) }
            td { (unit) }
            td class="num-right" { (format!("{:.2}", item.quantity)) }
            td class="num-right" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (format!("{:.2}", item.amount)) }
            td class="num-right" { (if item.received_qty > Decimal::ZERO { format!("{:.2}", item.received_qty) } else { "—".into() }) }
            td class="num-right" { (if item.inspected_qty > Decimal::ZERO { format!("{:.2}", item.inspected_qty) } else { "—".into() }) }
            td class="num-right" { (if item.returned_qty > Decimal::ZERO { format!("{:.2}", item.returned_qty) } else { "—".into() }) }
            td { (expected_delivery) }
        }
    }
}
