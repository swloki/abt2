use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::enums::PaymentMethod;
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::payment::model::*;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::purchase::reconciliation::model::PurchaseReconciliationQuery;
use abt_core::shared::types::PageParams;
use abt_core::shared::types::DomainError;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::payment_request::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct PayCreateForm {
    pub supplier_id: i64,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub reconciliation_id: Option<i64>,
    pub payment_date: String,
    pub amount: String,
    pub payment_method: i16,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub bank_account_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub invoice_number: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub invoice_amount: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("PAYMENT_REQUEST", "create")]
pub async fn get_pay_create(
    _path: PayCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let supplier_svc = state.supplier_service();
    let recon_svc = state.purchase_reconciliation_service();

    let suppliers = supplier_svc
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                name: None,
                status: None,
                category: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let reconciliations = recon_svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseReconciliationQuery::default(),
            PageParams::new(1, 200),
        )
        .await?;

    let content = pay_create_page(&suppliers.items, &reconciliations.items);
    let page_html = admin_page(
        is_htmx,
        "新建付款申请",
        &claims,
        "purchase",
        PayCreatePath::PATH,
        "采购管理",
        Some("新建付款申请"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PAYMENT_REQUEST", "create")]
pub async fn create_pay(
    _path: PayCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PayCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.payment_request_service();

    let payment_date = chrono::NaiveDate::parse_from_str(&form.payment_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效付款日期格式: {e}")))?;

    let amount: rust_decimal::Decimal = form
        .amount
        .parse()
        .map_err(|e| DomainError::validation(format!("无效金额格式: {e}")))?;

    let payment_method = PaymentMethod::from_i16(form.payment_method)
        .ok_or_else(|| DomainError::validation("无效付款方式".to_string()))?;

    let invoice_amount = form
        .invoice_amount
        .and_then(|s| s.parse::<rust_decimal::Decimal>().ok());

    let create_req = CreatePaymentRequestRequest {
        supplier_id: form.supplier_id,
        reconciliation_id: form.reconciliation_id,
        payment_date,
        amount,
        payment_method,
        bank_account_id: form.bank_account_id,
        invoice_number: form.invoice_number,
        invoice_amount,
        remark: form.remark.unwrap_or_default(),
    };

    let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

    let redirect = PayDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pay_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    reconciliations: &[abt_core::purchase::reconciliation::model::PurchaseReconciliation],
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(PayListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回付款申请列表"
                }
                h1 class="page-title" { "新建付款申请" }
            }

            form id="pay-form"
                  hx-post=(PayCreatePath::PATH)
                  hx-swap="none" {

            // ── Supplier & Reconciliation ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "供应商与对账信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "供应商" span style="color:var(--danger)" { "*" } }
                        select name="supplier_id" required {
                            option value="" disabled selected { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "关联对账单" }
                        select name="reconciliation_id" {
                            option value="" { "无" }
                            @for r in reconciliations {
                                option value=(r.id) { (r.doc_number) " — " (r.period) }
                            }
                        }
                    }
                }
            }

            // ── Payment Info ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "付款信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "付款日期" span style="color:var(--danger)" { "*" } }
                        input type="date" name="payment_date" value=(today) {}
                    }
                    div class="form-field" {
                        label { "付款金额" span style="color:var(--danger)" { "*" } }
                        input type="number" name="amount" step="0.01" min="0" placeholder="0.00" required {}
                    }
                    div class="form-field" {
                        label { "付款方式" span style="color:var(--danger)" { "*" } }
                        select name="payment_method" required {
                            option value="1" selected { "银行转账" }
                            option value="2" { "现金" }
                            option value="3" { "票据" }
                        }
                    }
                    div class="form-field" {
                        label { "银行账户" }
                        input type="number" name="bank_account_id" placeholder="银行账户ID" {}
                    }
                }
            }

            // ── Invoice Info ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "发票信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "发票号" }
                        input type="text" name="invoice_number" placeholder="输入发票号" {}
                    }
                    div class="form-field" {
                        label { "发票金额" }
                        input type="number" name="invoice_amount" step="0.01" min="0" placeholder="0.00" {}
                    }
                }
            }

            // ── Remark ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "备注" }
                textarea name="remark" placeholder="输入付款申请相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
            }

            // ── Action Bar ──
            div class="create-action-bar" {
                a class="btn btn-default" href=(PayListPath::PATH) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="submit" class="btn btn-primary" {
                        "提交付款申请"
                    }
                }
            }
            }
        }
    }
}
