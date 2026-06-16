use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::quotation::model::*;
use abt_core::sales::quotation::QuotationService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::quotation::*;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
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

pub async fn get_quotation_detail(
    path: QuotationDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.quotation_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let quotation = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let items = svc.list_items(&service_ctx, &mut conn, path.id).await?;

    let customer_name = customer_svc.get(&service_ctx, &mut conn, quotation.customer_id).await.map(|c| c.name).unwrap_or_else(|_| "未知客户".into());

    // 联系人信息
    let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, quotation.customer_id).await.unwrap_or_default();
    let contact = contacts.iter().find(|c| c.id == quotation.contact_id);
    let contact_name = contact.map(|c| c.name.as_str()).unwrap_or("—");
    let contact_phone = contact.and_then(|c| c.phone.as_deref()).unwrap_or("—");

    // 业务员信息
    let sales_rep_name = user_svc.get_user(&service_ctx, &mut conn, quotation.sales_rep_id)
        .await.map(|u| u.display_name.unwrap_or_else(|| u.username.clone()))
        .unwrap_or_else(|_| "—".into());

    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let products = if !product_ids.is_empty() {
        product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
    } else { vec![] };
    let product_names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
    let product_codes: HashMap<i64, String> = products.into_iter().map(|p| (p.product_id, p.product_code)).collect();

    let content = quotation_detail_page(
        &quotation, &items, &customer_name, contact_name, contact_phone,
        &sales_rep_name, &product_names, &product_codes,
    );
    let page_html = admin_page(
        is_htmx, "报价单详情", &claims, "sales",
        &format!("{}/{}", QuotationListPath::PATH, path.id),
        "销售管理", Some("报价单详情"), content, &nav_filter,
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
    contact_name: &str,
    contact_phone: &str,
    sales_rep_name: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(q.status);
    let is_draft = q.status == QuotationStatus::Draft;
    let is_sent = q.status == QuotationStatus::Sent;
    let is_accepted = q.status == QuotationStatus::Accepted;

    html! {
        div {
            // ── Back Link ──
            a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", QuotationListPath::PATH)) {
                (icon::arrow_left_icon("w-4 h-4"))
                "返回报价单列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div class="detail-title-row" {
                    h1 class="detail-no font-mono" { (q.doc_number) }
                    span class=(format!("status-pill {status_class}")) { (status_text) }
                }
                div class="flex gap-3" {
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
                    @if is_accepted {
                        button class="btn btn-default" onclick="window.print()" {
                            (icon::printer_icon("w-4 h-4"))
                            "打印"
                        }
                        a class="btn btn-primary" href=(format!("/admin/orders/create?from_quotation={}", q.id)) {
                            (icon::arrow_right_icon("w-4 h-4"))
                            "转销售订单"
                        }
                    }
                }
            }

            // ── Basic Info Card ──
            div class="info-card" {
                div class="info-card-title" { "基本信息" }
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
                        span class="info-value" { (sales_rep_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "报价日期" }
                        span class="info-value mono" { (q.quotation_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "有效期至" }
                        span class="info-value mono" { (q.valid_until.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "付款条款" }
                        span class="info-value" { (q.payment_terms.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "交货条款" }
                        span class="info-value" { (q.delivery_terms.as_str()) }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th { "规格描述" }
                            th { "单位" }
                            th class="num-right" { "数量" }
                            th class="num-right" { "单价" }
                            th class="num-right" { "折扣" }
                            th class="num-right" { "小计" }
                            th { "交货日期" }
                        }
                    }
                    tbody {
                        @for item in items {
                            (item_row(item, product_names, product_codes))
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
                div class="amount-summary" {
                    div class="amount-row" {
                        span class="amount-label" { "成本合计" }
                        span class="amount-value" {
                            (crate::utils::fmt_amount(q.total_cost))
                        }
                    }
                    div class="amount-row" {
                        span class="amount-label" { "预估利润" }
                        span class="amount-value text-success" {
                            (format!("{:.1}%", q.estimated_margin * rust_decimal::Decimal::ONE_HUNDRED))
                        }
                    }
                    div class="amount-row" {
                        span class="amount-label" { "报价总额" }
                        span class="amount-value accent" {
                            (crate::utils::fmt_amount(q.total_amount))
                        }
                    }
                }
            }

            // ── Remark ──
            @if !q.remark.is_empty() {
                div class="info-card mt-6" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (q.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(item: &QuotationItem, names: &HashMap<i64, String>, codes: &HashMap<i64, String>) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());

    let discount = if item.discount_rate > rust_decimal::Decimal::ZERO {
        format!("{}%", fmt_qty(item.discount_rate))
    } else {
        "0%".into()
    };

    html! {
        tr {
            td class="mono" { (item.line_no) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (item.description.as_str()) }
            td { (item.unit.as_str()) }
            td class="num-right" { (fmt_qty(item.quantity)) }
            td class="num-right" { (crate::utils::fmt_amount(item.unit_price)) }
            td class="num-right" { (discount) }
            td class="num-right" { (crate::utils::fmt_amount(item.amount)) }
            td class="mono" { (delivery) }
        }
    }
}
