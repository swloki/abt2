use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::identity::UserService;
use abt_core::wms::inventory::InventoryService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::routes::shipping::ShippingCreatePath;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: SalesOrderStatus) -> (&'static str, &'static str) {
    match s {
        SalesOrderStatus::Draft => ("草稿", "status-draft"),
        SalesOrderStatus::Confirmed => ("已确认", "status-confirmed"),
        SalesOrderStatus::InProduction => ("生产中", "status-progress"),
        SalesOrderStatus::PartiallyShipped => ("部分发货", "status-partial"),
        SalesOrderStatus::Shipped => ("已发货", "status-shipped"),
        SalesOrderStatus::Completed => ("已完成", "status-completed"),
        SalesOrderStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

struct ContactInfo {
    name: String,
    phone: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_detail(
    path: OrderDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();
    let inventory_svc = state.inventory_service();

    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let customer_name = customer_svc
        .get(&service_ctx, &mut conn, order.customer_id)
        .await
        .map(|c| c.name)
        .unwrap_or_else(|_| "未知客户".into());

    let contact = {
        let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, order.customer_id).await.unwrap_or_default();
        contacts.into_iter().find(|c| c.id == order.contact_id).map(|c| ContactInfo {
            name: c.name,
            phone: c.phone,
        })
    };

    let sales_rep = user_svc
        .get_user(&service_ctx, &mut conn, order.sales_rep_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let (product_names, product_codes) = {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if product_ids.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
            let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
            let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
            (names, codes)
        }
    };

    let stock_quantities: HashMap<i64, rust_decimal::Decimal> = {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        let mut map = HashMap::new();
        for pid in product_ids {
            let total = inventory_svc
                .get_by_product(&service_ctx, &mut conn, pid)
                .await
                .unwrap_or_default()
                .into_iter()
                .fold(rust_decimal::Decimal::ZERO, |acc, v| acc + v.quantity);
            map.insert(pid, total);
        }
        map
    };

    let content = order_detail_page(&order, &items, &customer_name, &contact, &sales_rep, &product_names, &product_codes, &stock_quantities);
    let page_html = admin_page(
        is_htmx, "订单详情", &claims, "sales",
        &format!("{}/{}", OrderListPath::PATH, path.id),
        "销售管理", Some("订单详情"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_order(
    path: ConfirmOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.confirm(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn start_order(
    path: StartOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.start_progress(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn complete_order(
    path: CompleteOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.complete(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn cancel_order(
    path: CancelOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.cancel(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: SalesOrderStatus) -> Markup {
    let steps: &[(&str, SalesOrderStatus)] = &[
        ("草稿", SalesOrderStatus::Draft),
        ("已确认", SalesOrderStatus::Confirmed),
        ("生产中", SalesOrderStatus::InProduction),
        ("部分发货", SalesOrderStatus::PartiallyShipped),
        ("已发货", SalesOrderStatus::Shipped),
        ("已完成", SalesOrderStatus::Completed),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == SalesOrderStatus::Cancelled;

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
                div class="wf-step danger" {
                    span class="wf-dot" {}
                    "已取消"
                }
            }
        }
    }
}

// ── Components ──

fn order_detail_page(
    o: &SalesOrder,
    items: &[SalesOrderItem],
    customer_name: &str,
    contact: &Option<ContactInfo>,
    sales_rep: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
    stock_quantities: &HashMap<i64, rust_decimal::Decimal>,
) -> Markup {
    let (status_text, status_class) = status_label(o.status);
    let contact_name = contact.as_ref().map(|c| c.name.as_str()).unwrap_or("—");
    let contact_phone = contact.as_ref().and_then(|c| c.phone.as_deref()).unwrap_or("—");

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(OrderListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回销售订单列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (o.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    button class="btn btn-default" {
                        (icon::printer_icon("w-4 h-4"))
                        "打印"
                    }
                    @if matches!(o.status, SalesOrderStatus::Confirmed | SalesOrderStatus::InProduction | SalesOrderStatus::PartiallyShipped) {
                        a class="btn btn-primary" href=(format!("{}?order_id={}", ShippingCreatePath::PATH, o.id)) {
                            (icon::truck_icon("w-4 h-4"))
                            "创建发货申请"
                        }
                    }
                    @if o.status == SalesOrderStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(ConfirmOrderPath { id: o.id }.to_string())
                            hx-confirm="确认审核此订单？" { "确认订单" }
                    }
                    @if o.status == SalesOrderStatus::Confirmed {
                        button class="btn btn-primary"
                            hx-post=(StartOrderPath { id: o.id }.to_string())
                            hx-confirm="确认开始生产？" { "开始生产" }
                    }
                    @if o.status == SalesOrderStatus::InProduction {
                        button class="btn btn-success"
                            hx-post=(CompleteOrderPath { id: o.id }.to_string())
                            hx-confirm="确认完成此订单？" { "完成订单" }
                    }
                    @if matches!(o.status, SalesOrderStatus::Draft | SalesOrderStatus::Confirmed) {
                        button class="btn btn-danger"
                            hx-post=(CancelOrderPath { id: o.id }.to_string())
                            hx-confirm="确认取消此订单？取消后不可恢复。" { "取消订单" }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(o.status))

            // ── Order Info ──
            div class="info-card" {
                div class="info-card-title" { "订单信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户名称" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "联系人" }
                        span class="info-value" { (contact_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "联系电话" }
                        span class="info-value mono" { (contact_phone) }
                    }
                    div class="info-item" {
                        span class="info-label" { "业务员" }
                        span class="info-value" { (sales_rep) }
                    }
                    div class="info-item" {
                        span class="info-label" { "交货日期" }
                        span class="info-value mono" { (o.order_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款条款" }
                        span class="info-value" { (o.payment_terms.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "交货条款" }
                        span class="info-value" { (o.delivery_terms.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "交货地址" }
                        span class="info-value" { (o.delivery_address.as_str()) }
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
                                th class="num-right" { "数量" }
                                th class="num-right" { "库存数量" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "折扣" }
                                th class="num-right" { "小计" }
                                th class="num-right" { "已发货" }
                                th class="num-right" { "已退货" }
                                th { "交货日期" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names, product_codes, stock_quantities))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="12" class="td-empty" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
                div class="amount-summary" {
                    div class="amount-row" {
                        span class="amount-label" { "成本合计" }
                        span class="amount-value" { (crate::utils::fmt_amount(o.total_cost)) }
                    }
                    div class="amount-row" {
                        span class="amount-label" { "订单总额" }
                        span class="amount-value accent" { (crate::utils::fmt_amount(o.total_amount)) }
                    }
                }
            }

            // ── Remarks ──
            @if !o.remark.is_empty() {
                div class="info-card mt-6" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (o.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &SalesOrderItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
    stock_quantities: &HashMap<i64, rust_decimal::Decimal>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
    let discount = if item.discount_rate > rust_decimal::Decimal::ZERO {
        format!("{}%", fmt_qty(item.discount_rate))
    } else {
        "—".into()
    };
    let stock_qty = stock_quantities.get(&item.product_id).copied().unwrap_or(rust_decimal::Decimal::ZERO);
    let stock_display = if stock_qty > rust_decimal::Decimal::ZERO { fmt_qty(stock_qty) } else { "—".into() };

    html! {
        tr {
            td class="mono" { (item.line_no) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (item.unit.as_str()) }
            td class="num-right" { (fmt_qty(item.quantity)) }
            td class="num-right" { (stock_display) }
            td class="num-right" { (crate::utils::fmt_amount(item.unit_price)) }
            td class="num-right" { (discount) }
            td class="num-right" { (crate::utils::fmt_amount(item.amount)) }
            td class="num-right" { (fmt_qty(item.shipped_qty)) }
            td class="num-right" { (fmt_qty(item.returned_qty)) }
            td class="mono" { (delivery) }
        }
    }
}
