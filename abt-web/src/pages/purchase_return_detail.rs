use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseReturnStatus;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::return_order::model::*;
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_return::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PurchaseReturnStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseReturnStatus::Draft => ("草稿", "status-draft"),
        PurchaseReturnStatus::Confirmed => ("已确认", "status-confirmed"),
        PurchaseReturnStatus::Shipped => ("已发货", "status-shipped"),
        PurchaseReturnStatus::Settled => ("已结算", "status-completed"),
        PurchaseReturnStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_RETURN", "read")]
pub async fn get_pr_detail(
    path: PRDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();
    let supplier_svc = state.supplier_service();
    let order_svc = state.purchase_order_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let pr = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, pr.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let order_doc_number = order_svc
        .get(&service_ctx, &mut conn, pr.order_id)
        .await
        .map(|o| o.doc_number)
        .ok();

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, pr.operator_id)
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

    let content = pr_detail_page(&pr, &items, &supplier_name, &order_doc_number, &operator_name, &product_names, &product_codes);
    let page_html = admin_page(
        is_htmx, "退货详情", &claims, "purchase",
        &format!("{}/{}", PRListPath::PATH, path.id),
        "采购管理", Some("退货详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_RETURN", "update")]
pub async fn confirm_pr(
    path: PRConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();

    svc.confirm(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PRDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_RETURN", "update")]
pub async fn cancel_pr(
    path: PRCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();

    svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PRDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: PurchaseReturnStatus) -> Markup {
    let steps: &[(&str, PurchaseReturnStatus)] = &[
        ("草稿", PurchaseReturnStatus::Draft),
        ("已确认", PurchaseReturnStatus::Confirmed),
        ("已发货", PurchaseReturnStatus::Shipped),
        ("已结算", PurchaseReturnStatus::Settled),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == PurchaseReturnStatus::Cancelled;

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

fn pr_detail_page(
    pr: &PurchaseReturn,
    items: &[PurchaseReturnItem],
    supplier_name: &str,
    order_doc_number: &Option<String>,
    operator_name: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(pr.status);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(PRListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回采购退货列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (pr.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    @if pr.status == PurchaseReturnStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(PRConfirmPath { id: pr.id }.to_string())
                            hx-confirm="确认此退货单？确认后将执行退货。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认退货"
                        }
                        button class="btn btn-danger"
                            hx-post=(PRCancelPath { id: pr.id }.to_string())
                            hx-confirm="确认取消此退货单？取消后不可恢复。" {
                            "取消"
                        }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(pr.status))

            // ── Return Info ──
            div class="info-card" {
                div class="info-card-title" { "退货信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "供应商名称" }
                        span class="info-value" { (supplier_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "关联订单" }
                        span class="info-value mono" {
                            @if let Some(doc) = order_doc_number {
                                (doc)
                            } @else {
                                "—"
                            }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "退货日期" }
                        span class="info-value mono" { (pr.return_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "退货原因" }
                        span class="info-value" { (pr.return_reason.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作人" }
                        span class="info-value" { (operator_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value mono" { (pr.created_at.format("%Y-%m-%d %H:%M")) }
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
                                th class="num-right" { "退货数量" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "小计" }
                            }
                        }
                        tbody {
                            @for (idx, item) in items.iter().enumerate() {
                                (item_row(idx, item, product_names, product_codes))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="6" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Amount Summary ──
            div class="info-card" style="margin-top:var(--space-6)" {
                div class="info-card-title" { "退货总额" }
                div class="amount-summary" {
                    span class="amount-label" { "退货总额" }
                    span class="amount-value" { (format!("{:.2}", pr.total_amount)) }
                }
            }
        }
    }
}

fn item_row(
    idx: usize,
    item: &PurchaseReturnItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td class="mono" { (idx + 1) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td class="num-right" { (item.returned_qty) }
            td class="num-right" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (format!("{:.2}", item.amount)) }
        }
    }
}
