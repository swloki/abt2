use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseQuotationStatus;
use abt_core::purchase::quotation::model::*;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::PODetailPath;
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
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_delete = ctx.has_permission("PURCHASE_QUOTATION", "delete").await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_quotation_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let pq = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let supplier = supplier_svc
        .get(&service_ctx, &mut conn, pq.supplier_id)
        .await
        .ok();
    let supplier_name = supplier.as_ref().map(|s| s.name.as_str()).unwrap_or("未知供应商");

    // Resolve primary contact for supplier
    let (supplier_contact, supplier_phone) = match &supplier {
        Some(_s) => {
            let contacts = supplier_svc
                .list_contacts(&service_ctx, &mut conn, pq.supplier_id)
                .await
                .unwrap_or_default();
            let primary = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());
            match primary {
                Some(c) => (c.name.clone(), c.phone.clone().unwrap_or_else(|| "—".into())),
                None => ("—".into(), "—".into()),
            }
        }
        None => ("—".into(), "—".into()),
    };

    let buyer_name = user_svc
        .get_user(&service_ctx, &mut conn, pq.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let (product_names, product_codes, product_specs, product_units) = {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if product_ids.is_empty() {
            (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new())
        } else {
            let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
            let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
            let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
            let specs: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.meta.specification.clone())).collect();
            let units: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.unit.clone())).collect();
            (names, codes, specs, units)
        }
    };

    let qctx = QuotationDetailContext {
        supplier_name,
        supplier_contact: &supplier_contact,
        supplier_phone: &supplier_phone,
        buyer_name: &buyer_name,
        product_names: &product_names,
        product_codes: &product_codes,
        product_specs: &product_specs,
        product_units: &product_units,
        can_delete,
    };
    let content = pq_detail_page(&pq, &items, &qctx);
    let page_html = admin_page(
        is_htmx, "报价详情", &claims, "purchase",
        &format!("{}/{}", PQListPath::PATH, path.id),
        "采购管理", Some("报价详情"), content, &nav_filter,
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

#[require_permission("PURCHASE_QUOTATION", "delete")]
pub async fn delete_pq(
    path: PQDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_quotation_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", PQListPath.to_string())], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "create")]
pub async fn convert_pq(
    path: PQConvertPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let order_svc = state.purchase_order_service();

    let order_id = order_svc.create_from_quotation(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PODetailPath { id: order_id }.to_string();
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

struct QuotationDetailContext<'a> {
    supplier_name: &'a str,
    supplier_contact: &'a str,
    supplier_phone: &'a str,
    buyer_name: &'a str,
    product_names: &'a HashMap<i64, String>,
    product_codes: &'a HashMap<i64, String>,
    product_specs: &'a HashMap<i64, String>,
    product_units: &'a HashMap<i64, String>,
    can_delete: bool,
}

fn pq_detail_page(
    pq: &PurchaseQuotation,
    items: &[PurchaseQuotationItem],
    ctx: &QuotationDetailContext,
) -> Markup {
    let (status_text, status_class) = status_label(pq.status);
    let currency = items.first().map(|i| i.currency.as_str()).unwrap_or("CNY");
    let remark = if pq.remark.is_empty() { "—" } else { &pq.remark };
    html! {
        div {
            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (pq.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="flex gap-3" {
                    button class="btn bg-white text-fg border border-border hover:bg-surface" { "打印" }
                    @if pq.status == PurchaseQuotationStatus::Active {
                        button class="btn bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-post=(PQConvertPath { id: pq.id }.to_string())
                            hx-confirm="确认将此报价单转为采购订单？" {
                            "转采购订单"
                        }
                    }
                    @if pq.status == PurchaseQuotationStatus::Draft {
                        button class="btn bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-post=(PQActivatePath { id: pq.id }.to_string())
                            hx-confirm="确认激活此报价？激活后将生效。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "激活报价"
                        }
                        button class="btn bg-danger text-white border-none hover:opacity-90"
                            hx-post=(PQCancelPath { id: pq.id }.to_string())
                            hx-confirm="确认取消此报价？取消后不可恢复。" {
                            "取消"
                        }
                    }
                    @if pq.status != PurchaseQuotationStatus::Active && ctx.can_delete {
                        button class="btn bg-danger text-white border-none hover:opacity-90-ghost"
                            hx-post=(PQDeletePath { id: pq.id }.to_string())
                            hx-confirm="确认删除此报价？删除后不可恢复。" {
                            (icon::trash_icon("w-4 h-4"))
                            "删除"
                        }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(pq.status))

            // ── Quotation Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "报价信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "供应商名称" }
                        span class="text-sm text-fg font-medium" { (ctx.supplier_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "联系人" }
                        span class="text-sm text-fg font-medium" { (ctx.supplier_contact) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "联系电话" }
                        span class="text-sm text-fg font-medium" { (ctx.supplier_phone) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "报价日期" }
                        span class="text-sm text-fg font-medium mono" { (pq.quotation_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "有效期" }
                        span class="text-sm text-fg font-medium mono" {
                            (format!("{} ~ {}", pq.valid_from.format("%Y-%m-%d"), pq.valid_until.format("%Y-%m-%d")))
                        }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "币种" }
                        span class="text-sm text-fg font-medium" { (currency) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "采购员" }
                        span class="text-sm text-fg font-medium" { (ctx.buyer_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "备注" }
                        span class="text-sm text-fg font-medium" { (remark) }
                    }
                }
            }

            // ── Items Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "物料编码" }
                                th { "物料名称" }
                                th { "规格描述" }
                                th { "单位" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "最小起订量" }
                                th { "交货周期" }
                                th { "是否首选" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, ctx.product_names, ctx.product_codes, ctx.product_specs, ctx.product_units))
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
            }

            // ── Amount Summary ──
            div class="amount-summary" {
                div class="amount-row" {
                    span class="amount-label" { "报价项目" }
                    span class="amount-value" { (format!("{} 项", items.len())) }
                }
                div class="amount-row" {
                    span class="amount-label" { "首选供应商" }
                    span class="amount-value accent" {
                        (format!("{} 项", items.iter().filter(|i| i.is_preferred).count()))
                    }
                }
            }
        }
    }
}

fn item_row(
    item: &PurchaseQuotationItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
    specs: &HashMap<i64, String>,
    units: &HashMap<i64, String>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let spec = specs.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let unit = units.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let min_qty = item.min_order_qty.map(|q| format!("{:.2}", q)).unwrap_or_else(|| "—".into());
    let lead_time = item.lead_time_days.map(|d| d.to_string()).unwrap_or_else(|| "—".into());
    let preferred = if item.is_preferred { "✓" } else { "—" };

    html! {
        tr {
            td class="mono" { (item.line_no) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (spec) }
            td { (unit) }
            td class="num-right" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (min_qty) }
            td { (lead_time) }
            td style="text-align:center" { (preferred) }
        }
    }
}
