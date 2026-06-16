use std::collections::{HashMap, HashSet};

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::sales::reconciliation::model::*;
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::routes::order::OrderDetailPath;
use crate::routes::shipping::ShippingDetailPath;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: ReconciliationStatus) -> (&'static str, &'static str) {
    match s {
        ReconciliationStatus::Draft => ("草稿", "status-draft"),
        ReconciliationStatus::Sent => ("已发送", "status-sent"),
        ReconciliationStatus::Confirmed => ("已确认", "status-confirmed"),
        ReconciliationStatus::Disputed => ("有异议", "status-disputed"),
        ReconciliationStatus::Settled => ("已结算", "status-settled"),
    }
}

struct ProductDetail {
    code: String,
    name: String,
    unit: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_reconciliation_detail(
    path: ReconciliationDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let reconciliation_svc = state.reconciliation_service();
    let customer_svc = state.customer_service();
    let order_svc = state.sales_order_service();
    let shipping_svc = state.shipping_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let rec = reconciliation_svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let items = reconciliation_svc.list_items(&service_ctx, &mut conn, path.id).await?;

    let customer_name = customer_svc
        .get(&service_ctx, &mut conn, rec.customer_id)
        .await
        .map(|c| c.name)
        .unwrap_or_else(|_| "未知客户".into());

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, rec.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    // Resolve product details via service
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_details: HashMap<i64, ProductDetail> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .map(|products| {
                products
                    .into_iter()
                    .map(|p| {
                        (
                            p.product_id,
                            ProductDetail {
                                code: p.product_code,
                                name: p.pdt_name,
                                unit: Some(p.unit),
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    // Resolve order numbers via service (deduplicated)
    let order_numbers: HashMap<i64, String> = {
        let mut map = HashMap::new();
        let mut seen = HashSet::new();
        for item in &items {
            if seen.insert(item.sales_order_id)
                && let Ok(order) = order_svc.find_by_id(&service_ctx, &mut conn, item.sales_order_id).await {
                    map.insert(item.sales_order_id, order.doc_number);
                }
        }
        map
    };

    // Resolve shipping numbers via service (deduplicated)
    let shipping_numbers: HashMap<i64, String> = {
        let mut map = HashMap::new();
        let mut seen = HashSet::new();
        for item in &items {
            if seen.insert(item.shipping_request_id)
                && let Ok(shipping) = shipping_svc.find_by_id(&service_ctx, &mut conn, item.shipping_request_id).await {
                    map.insert(item.shipping_request_id, shipping.doc_number);
                }
        }
        map
    };

    let content = reconciliation_detail_page(&rec, &items, &customer_name, &operator_name, &product_details, &order_numbers, &shipping_numbers);
    let page_html = admin_page(
        is_htmx, "对账详情", &claims, "sales",
        &format!("{}/{}", ReconciliationListPath::PATH, path.id),
        "销售管理", Some("对账详情"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn send_reconciliation(
    path: SendReconciliationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let reconciliation_svc = state.reconciliation_service();
    reconciliation_svc.send(&service_ctx, &mut conn, path.id).await?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_reconciliation(
    path: ConfirmReconciliationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let reconciliation_svc = state.reconciliation_service();
    reconciliation_svc.confirm(&service_ctx, &mut conn, path.id).await?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn dispute_reconciliation(
    path: DisputeReconciliationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let reconciliation_svc = state.reconciliation_service();
    reconciliation_svc.dispute(&service_ctx, &mut conn, path.id).await?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn settle_reconciliation(
    path: SettleReconciliationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let reconciliation_svc = state.reconciliation_service();
    reconciliation_svc.settle(&service_ctx, &mut conn, path.id).await?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: ReconciliationStatus) -> Markup {
    let steps: &[(&str, ReconciliationStatus)] = &[
        ("草稿", ReconciliationStatus::Draft),
        ("已发送", ReconciliationStatus::Sent),
        ("已确认", ReconciliationStatus::Confirmed),
        ("已结算", ReconciliationStatus::Settled),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_disputed = current == ReconciliationStatus::Disputed;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx && !is_disputed { "wf-line current" } else { "wf-line" }) {}
                }
                @let step_class = if is_disputed {
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
            @if is_disputed {
                div class="wf-step disputed" {
                    div class="wf-dot" {}
                    "有异议"
                }
            }
        }
    }
}

// ── Components ──

fn reconciliation_detail_page(
    rec: &Reconciliation,
    items: &[ReconciliationItem],
    customer_name: &str,
    operator_name: &str,
    product_details: &HashMap<i64, ProductDetail>,
    order_numbers: &HashMap<i64, String>,
    shipping_numbers: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(rec.status);

    html! {
        div {
            // ── Back Link ──
            a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ReconciliationListPath::PATH)) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回对账单列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (rec.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                    div class="detail-source" {
                        "对账期间：" (rec.period.as_str())
                        "　客户：" (customer_name)
                    }
                }
                div class="flex gap-3" {
                    @if rec.status == ReconciliationStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(SendReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认发送此对账单？" { "发送对账" }
                    }
                    @if rec.status == ReconciliationStatus::Sent {
                        button class="btn btn-success"
                            hx-post=(ConfirmReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认此对账单？" { "确认" }
                        button class="btn btn-danger"
                            hx-post=(DisputeReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认提出异议？" { "异议" }
                    }
                    @if rec.status == ReconciliationStatus::Confirmed {
                        button class="btn btn-success"
                            hx-post=(SettleReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认结算？" { "结算" }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(rec.status))

            // ── Summary Cards ──
            div class="grid grid-cols-3 gap-4 mb-6" {
                div class="data-card stat-mini" {
                    div class="info-label" { "总金额" }
                    div class="mono stat-mini-value" {
                        (crate::utils::fmt_amount(rec.total_amount))
                    }
                }
                div class="data-card stat-mini" {
                    div class="info-label" { "确认金额" }
                    div class="mono stat-mini-value text-success" {
                        (crate::utils::fmt_amount(rec.confirmed_amount))
                    }
                }
                div class="data-card stat-mini" {
                    div class="info-label" { "差额" }
                    div class="mono stat-mini-value text-danger" {
                        (crate::utils::fmt_amount(rec.difference))
                    }
                }
            }

            // ── Info ──
            div class="info-card" {
                div class="info-card-title" { "对账信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户名称" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "对账期间" }
                        span class="info-value" { (rec.period.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { (operator_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value" { (rec.created_at.format("%Y-%m-%d %H:%M")) }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "来源类型" }
                            th { "来源单号" }
                            th { "关联订单" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th { "单位" }
                            th class="num-right" { "数量" }
                            th class="num-right" { "单价" }
                            th class="num-right" { "金额" }
                            th { "确认" }
                        }
                    }
                    tbody {
                        @for item in items {
                            (item_row(item, product_details, order_numbers, shipping_numbers))
                        }
                        @if items.is_empty() {
                            tr {
                                td colspan="10" class="td-empty" {
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
                    span class="amount-label" { "确认金额" }
                    span class="amount-value text-success" {
                        (crate::utils::fmt_amount(rec.confirmed_amount))
                    }
                }
                div class="amount-row" {
                    span class="amount-label" { "差异金额" }
                    span class="amount-value" {
                        (crate::utils::fmt_amount(rec.difference))
                    }
                }
                div class="amount-row" {
                    span class="amount-label" { "对账净额" }
                    span class="amount-value accent" {
                        (crate::utils::fmt_amount(rec.total_amount))
                    }
                }
            }

            // ── Remarks ──
            @if !rec.remark.is_empty() {
                div class="info-card mt-6" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (rec.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &ReconciliationItem,
    product_details: &HashMap<i64, ProductDetail>,
    order_numbers: &HashMap<i64, String>,
    shipping_numbers: &HashMap<i64, String>,
) -> Markup {
    let detail = product_details.get(&item.product_id);
    let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
    let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
    let unit = detail.and_then(|d| d.unit.as_deref()).unwrap_or("—");
    let order_num = order_numbers.get(&item.sales_order_id).map(|s| s.as_str()).unwrap_or("—");
    let shipping_num = shipping_numbers.get(&item.shipping_request_id).map(|s| s.as_str()).unwrap_or("—");
    let shipping_detail = ShippingDetailPath { id: item.shipping_request_id };
    let order_detail = OrderDetailPath { id: item.sales_order_id };

    html! {
        tr {
            td { "发货" }
            td {
                a href=(shipping_detail.to_string()) class="link-color-info" { (shipping_num) }
            }
            td {
                a href=(order_detail.to_string()) class="link-color-info" { (order_num) }
            }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (unit) }
            td class="num-right" { (fmt_qty(item.quantity)) }
            td class="num-right mono" { (format!("{:.2}", item.unit_price)) }
            td class="num-right mono" { (format!("{:.2}", item.amount)) }
            td {
                @if item.confirmed {
                    span class="text-success" { "已确认" }
                } @else {
                    span class="text-muted" { "未确认" }
                }
            }
        }
    }
}
