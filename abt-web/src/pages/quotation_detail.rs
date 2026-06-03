use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::quotation::model::*;
use abt_core::sales::quotation::QuotationService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::quotation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: QuotationStatus) -> (&'static str, &'static str) {
    match s {
        QuotationStatus::Draft => ("草稿", "status-draft"),
        QuotationStatus::Sent => ("已发送", "status-sent"),
        QuotationStatus::Accepted => ("已接受", "status-accepted"),
        QuotationStatus::Rejected => ("已拒绝", "status-rejected"),
        QuotationStatus::Expired => ("已过期", "status-expired"),
    }
}


// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_quotation_detail(
    path: QuotationDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.quotation_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();

    let quotation = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let items = svc.list_items(&service_ctx, &mut conn, path.id).await?;

    let customer_name = customer_svc.get(&service_ctx, &mut conn, quotation.customer_id).await.map(|c| c.name).unwrap_or_else(|_| "未知客户".into());
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let products = if !product_ids.is_empty() {
        product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
    } else { vec![] };
    let product_names: HashMap<i64, String> = products.into_iter().map(|p| (p.product_id, p.pdt_name)).collect();

    let content = quotation_detail_page(&quotation, &items, &customer_name, &product_names);
    let page_html = admin_page(
        is_htmx, "报价单详情", &claims, "sales",
        &format!("{}/{}", QuotationListPath::PATH, path.id),
        "销售管理", Some("报价单详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn submit_quotation(
    path: SubmitQuotationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    svc.submit(&service_ctx, &mut conn, path.id).await?;

    let redirect = QuotationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn accept_quotation(
    path: AcceptQuotationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    svc.accept(&service_ctx, &mut conn, path.id).await?;

    let redirect = QuotationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn reject_quotation(
    path: RejectQuotationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    svc.reject(&service_ctx, &mut conn, path.id).await?;

    let redirect = QuotationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn quotation_detail_page(
    q: &Quotation,
    items: &[QuotationItem],
    customer_name: &str,
    product_names: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(q.status);
    let is_draft = q.status == QuotationStatus::Draft;
    let is_sent = q.status == QuotationStatus::Sent;

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "报价单详情" }
                div class="page-actions" {
                    a class="btn btn-default" href=(QuotationListPath::PATH) { "返回列表" }
                    @if is_draft {
                        button class="btn btn-primary"
                            hx-post=(SubmitQuotationPath { id: q.id }.to_string())
                            hx-confirm="确认提交报价单？" { "提交报价" }
                    }
                    @if is_sent {
                        button class="btn btn-success"
                            hx-post=(AcceptQuotationPath { id: q.id }.to_string())
                            hx-confirm="确认接受该报价？" { "接受" }
                        button class="btn btn-danger"
                            hx-post=(RejectQuotationPath { id: q.id }.to_string())
                            hx-confirm="确认拒绝该报价？" { "拒绝" }
                    }
                }
            }

            // ── Status Banner ──
            div class="data-card" style="margin-bottom:var(--space-6)" {
                div style="display:flex;align-items:center;gap:var(--space-4);margin-bottom:var(--space-4)" {
                    span class="mono" style="font-size:var(--text-xl);font-weight:600" { (q.doc_number) }
                    span class=(format!("status-pill {status_class}")) { (status_text) }
                }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "报价日期" }
                        span class="info-value" { (q.quotation_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "有效期至" }
                        span class="info-value" { (q.valid_until.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款条款" }
                        span class="info-value" { (q.payment_terms.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "交货条款" }
                        span class="info-value" { (q.delivery_terms.as_str()) }
                    }
                    @if !q.remark.is_empty() {
                        div class="info-item" {
                            span class="info-label" { "备注" }
                            span class="info-value" { (q.remark.as_str()) }
                        }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                div class="form-section-title" { "报价明细" }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "序号" }
                                th { "产品" }
                                th { "描述" }
                                th class="num-right" { "数量" }
                                th { "单位" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "折扣%" }
                                th class="num-right" { "金额" }
                                th { "交货日期" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
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
                        span { "总金额" }
                        span class="mono" style="font-size:var(--text-lg);font-weight:600" {
                            "¥ " (format!("{:.2}", q.total_amount))
                        }
                    }
                }
            }
        }
    }
}

fn item_row(item: &QuotationItem, names: &HashMap<i64, String>) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());

    html! {
        tr {
            td { (item.line_no) }
            td { (product_name) }
            td { (item.description.as_str()) }
            td class="num-right" { (item.quantity) }
            td { (item.unit.as_str()) }
            td class="num-right mono" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (item.discount_rate) }
            td class="num-right mono" { (format!("{:.2}", item.amount)) }
            td { (delivery) }
        }
    }
}
