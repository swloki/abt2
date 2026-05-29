use std::collections::HashMap;

use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseQuotationStatus;
use abt_core::purchase::quotation::model::*;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_quotation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PurchaseQuotationStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseQuotationStatus::Draft => ("草稿", "status-draft"),
        PurchaseQuotationStatus::Active => ("已生效", "status-confirmed"),
        PurchaseQuotationStatus::Expired => ("已过期", "status-progress"),
        PurchaseQuotationStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_QUOTATION", "read")]
pub async fn get_pq_detail(
    path: PQDetailPath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> Result<Html<String>> {
    let RequestContext { claims, mut conn, state, service_ctx } = ctx;
    let svc = state.purchase_quotation_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let pq = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, pq.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, pq.operator_id)
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

    let content = pq_detail_page(&pq, &items, &supplier_name, &operator_name, &product_names, &product_codes);
    let page_html = admin_page(
        &headers, "报价详情", &claims, "purchase",
        &format!("{}/{}", PQListPath::PATH, path.id),
        "采购管理", Some("报价详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_QUOTATION", "update")]
pub async fn activate_pq(
    path: PQActivatePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_quotation_service();

    svc.activate(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PQDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_QUOTATION", "update")]
pub async fn cancel_pq(
    path: PQCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_quotation_service();

    svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PQDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: PurchaseQuotationStatus) -> Markup {
    let steps: &[(&str, PurchaseQuotationStatus)] = &[
        ("草稿", PurchaseQuotationStatus::Draft),
        ("已生效", PurchaseQuotationStatus::Active),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == PurchaseQuotationStatus::Cancelled;
    let is_expired = current == PurchaseQuotationStatus::Expired;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    @let line_class = if i <= current_idx && !is_cancelled { "wf-line completed" } else { "wf-line" };
                    div class=(line_class) {}
                }
                @let step_class = if is_cancelled || is_expired {
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
            @if is_expired {
                div class="wf-line completed" {}
                div class="wf-step completed" {
                    span class="wf-dot" {}
                    "已过期"
                }
            }
        }
    }
}

// ── Components ──

fn pq_detail_page(
    pq: &PurchaseQuotation,
    items: &[PurchaseQuotationItem],
    supplier_name: &str,
    operator_name: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(pq.status);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(PQListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回采购报价列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (pq.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    @if pq.status == PurchaseQuotationStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(PQActivatePath { id: pq.id }.to_string())
                            hx-confirm="确认激活此报价？激活后将生效。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "激活报价"
                        }
                        button class="btn btn-danger"
                            hx-post=(PQCancelPath { id: pq.id }.to_string())
                            hx-confirm="确认取消此报价？取消后不可恢复。" {
                            "取消"
                        }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(pq.status))

            // ── Quotation Info ──
            div class="info-card" {
                div class="info-card-title" { "报价信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "供应商名称" }
                        span class="info-value" { (supplier_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "报价日期" }
                        span class="info-value mono" { (pq.quotation_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "有效期从" }
                        span class="info-value mono" { (pq.valid_from.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "有效期至" }
                        span class="info-value mono" { (pq.valid_until.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作人" }
                        span class="info-value" { (operator_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value mono" { (pq.created_at.format("%Y-%m-%d %H:%M")) }
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
                                th class="num-right" { "单价" }
                                th class="num-right" { "最小起订量" }
                                th class="num-right" { "交货周期(天)" }
                                th { "币种" }
                                th { "首选" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names, product_codes))
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

            // ── Remarks ──
            @if !pq.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-6)" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (pq.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &PurchaseQuotationItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let min_qty = item.min_order_qty.map(|q| q.to_string()).unwrap_or_else(|| "—".into());
    let lead_time = item.lead_time_days.map(|d| d.to_string()).unwrap_or_else(|| "—".into());
    let preferred = if item.is_preferred { "✓" } else { "—" };

    html! {
        tr {
            td class="mono" { (item.line_no) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td class="num-right" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (min_qty) }
            td class="num-right" { (lead_time) }
            td { (item.currency.as_str()) }
            td style="text-align:center" { (preferred) }
        }
    }
}
