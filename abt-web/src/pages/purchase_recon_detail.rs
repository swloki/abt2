use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseReconStatus;
use abt_core::purchase::reconciliation::model::*;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_reconciliation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PurchaseReconStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseReconStatus::Draft => ("草稿", "status-draft"),
        PurchaseReconStatus::Confirmed => ("已确认", "status-confirmed"),
        PurchaseReconStatus::Settled => ("已结算", "status-completed"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_RECON", "read")]
pub async fn get_precon_detail(
    path: PreconDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_reconciliation_service();
    let supplier_svc = state.supplier_service();
    let user_svc = state.user_service();

    let recon = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, recon.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, recon.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let content = precon_detail_page(&recon, &items, &supplier_name, &operator_name);
    let page_html = admin_page(
        is_htmx, "对账详情", &claims, "purchase",
        &format!("{}/{}", PreconListPath::PATH, path.id),
        "采购管理", Some("对账详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_RECON", "update")]
pub async fn confirm_precon(
    path: PreconConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_reconciliation_service();

    svc.confirm(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PreconDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn precon_detail_page(
    recon: &PurchaseReconciliation,
    items: &[PurchaseReconItem],
    supplier_name: &str,
    operator_name: &str,
) -> Markup {
    let (status_text, status_class) = status_label(recon.status);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(PreconListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回采购对账列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (recon.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    @if recon.status == PurchaseReconStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(PreconConfirmPath { id: recon.id }.to_string())
                            hx-confirm="确认对此对账单进行对账？确认后将不可修改。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认对账"
                        }
                    }
                }
            }

            // ── Reconciliation Info ──
            div class="info-card" {
                div class="info-card-title" { "对账信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "供应商名称" }
                        span class="info-value" { (supplier_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "对账期间" }
                        span class="info-value mono" { (recon.period) }
                    }
                    div class="info-item" {
                        span class="info-label" { "状态" }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作人" }
                        span class="info-value" { (operator_name) }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "订单ID" }
                                th { "订单明细ID" }
                                th class="num-right" { "收货数量" }
                                th class="num-right" { "退货数量" }
                                th class="num-right" { "退货冲减" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "金额" }
                                th { "已确认" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
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
                div class="info-card-title" { "金额汇总" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "总金额" }
                        span class="info-value mono" { (format!("{:.2}", recon.total_amount)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "确认金额" }
                        span class="info-value mono" { (format!("{:.2}", recon.confirmed_amount)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "差异" }
                        span class="info-value mono" { (format!("{:.2}", recon.difference)) }
                    }
                }
            }

            // ── Remarks ──
            @if !recon.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-6)" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (recon.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(item: &PurchaseReconItem) -> Markup {
    let confirmed = if item.confirmed { "✓" } else { "—" };

    html! {
        tr {
            td class="mono" { (item.order_id) }
            td class="mono" { (item.order_item_id) }
            td class="num-right" { (format!("{:.2}", item.received_qty)) }
            td class="num-right" { (format!("{:.2}", item.returned_qty)) }
            td class="num-right" { (format!("{:.2}", item.returned_amount)) }
            td class="num-right" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (format!("{:.2}", item.amount)) }
            td style="text-align:center" { (confirmed) }
        }
    }
}
