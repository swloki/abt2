use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::purchase::enums::{PaymentMethod, PaymentStatus};
use abt_core::purchase::payment::model::*;
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::payment_request::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PaymentStatus) -> (&'static str, &'static str) {
    match s {
        PaymentStatus::Draft => ("草稿", "status-draft"),
        PaymentStatus::Approved => ("已审批", "status-confirmed"),
        PaymentStatus::Paid => ("已支付", "status-completed"),
        PaymentStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

fn payment_method_label(m: PaymentMethod) -> &'static str {
    match m {
        PaymentMethod::BankTransfer => "银行转账",
        PaymentMethod::Cash => "现金",
        PaymentMethod::Note => "票据",
    }
}


// ── Workflow Steps ──

fn workflow_steps(current: PaymentStatus) -> Markup {
    let steps: &[(&str, PaymentStatus)] = &[
        ("草稿", PaymentStatus::Draft),
        ("已审批", PaymentStatus::Approved),
        ("已付款", PaymentStatus::Paid),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == PaymentStatus::Cancelled;

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

// ── Handlers ──

#[require_permission("PAYMENT_REQUEST", "read")]
pub async fn get_pay_detail(
    path: PayDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.payment_request_service();
    let supplier_svc = state.supplier_service();
    let recon_svc = state.purchase_reconciliation_service();
    let user_svc = state.user_service();

    let pay = svc.get(&service_ctx, &mut conn, path.id).await?;

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, pay.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let recon_doc_number = match pay.reconciliation_id {
        Some(rid) => recon_svc
            .get(&service_ctx, &mut conn, rid)
            .await
            .map(|r| r.doc_number)
            .ok(),
        None => None,
    };

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, pay.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let content = pay_detail_page(&pay, &supplier_name, recon_doc_number.as_deref(), &operator_name);
    let page_html = admin_page(
        is_htmx, "付款详情", &claims, "purchase",
        &format!("{}/{}", PayListPath::PATH, path.id),
        "采购管理", Some("付款详情"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PAYMENT_REQUEST", "update")]
pub async fn approve_pay(
    path: PayApprovePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.payment_request_service();

    svc.approve(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PayDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PAYMENT_REQUEST", "update")]
pub async fn cancel_pay(
    path: PayCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.payment_request_service();

    svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PayDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pay_detail_page(
    pay: &PaymentRequest,
    supplier_name: &str,
    recon_doc_number: Option<&str>,
    operator_name: &str,
) -> Markup {
    let (status_text, status_class) = status_label(pay.status);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(format!("{}?restore=true", PayListPath::PATH)) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回付款列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (pay.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    @if pay.status == PaymentStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(PayApprovePath { id: pay.id }.to_string())
                            hx-confirm="确认审批此付款申请？" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "审批"
                        }
                        button class="btn btn-danger"
                            hx-post=(PayCancelPath { id: pay.id }.to_string())
                            hx-confirm="确认取消此付款申请？取消后不可恢复。" {
                            "取消"
                        }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(pay.status))

            // ── Payment Info ──
            div class="info-card" {
                div class="info-card-title" { "付款信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "供应商名称" }
                        span class="info-value" { (supplier_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款日期" }
                        span class="info-value mono" { (pay.payment_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款方式" }
                        span class="info-value" { (payment_method_label(pay.payment_method)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款金额" }
                        span class="info-value mono" { (format!("{}", pay.amount)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "关联对账单" }
                        span class="info-value mono" {
                            @if let Some(doc) = recon_doc_number {
                                (doc)
                            } @else {
                                "—"
                            }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "银行账户" }
                        span class="info-value mono" {
                            @if let Some(bank_id) = pay.bank_account_id {
                                (format!("{}", bank_id))
                            } @else {
                                "—"
                            }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作人" }
                        span class="info-value" { (operator_name) }
                    }
                }
            }

            // ── Invoice Info ──
            div class="info-card" style="margin-top:var(--space-6)" {
                div class="info-card-title" { "发票信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "发票号" }
                        span class="info-value mono" {
                            @if let Some(ref inv) = pay.invoice_number {
                                (inv.as_str())
                            } @else {
                                "—"
                            }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "发票金额" }
                        span class="info-value mono" {
                            @if let Some(amt) = pay.invoice_amount {
                                (format!("{}", amt))
                            } @else {
                                "—"
                            }
                        }
                    }
                }
            }

            // ── Remarks ──
            @if !pay.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-6)" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (pay.remark.as_str()) }
                }
            }
        }
    }
}
