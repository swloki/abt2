use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::reconciliation::model::*;
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query & Form Structs ──

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    pub customer_id: Option<i64>,
    pub period: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReconciliationCreateForm {
    pub customer_id: i64,
    pub period: String,
    pub remark: Option<String>,
}

// ── Helpers ──

struct ProductInfo {
    code: String,
    name: String,
    unit: String,
}

fn generate_periods() -> Vec<(String, String)> {
    let now = chrono::Local::now();
    let mut periods = vec![];
    for i in 0..12 {
        let d = now - chrono::Months::new(i);
        let value = d.format("%Y-%m").to_string();
        let label = d.format("%Y年%m月").to_string();
        periods.push((value, label));
    }
    periods
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "create")]
pub async fn get_reconciliation_create(
    _path: ReconciliationCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let customer_svc = state.customer_service();
    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let content = reconciliation_create_page(&customers.items);
    let page_html = admin_page(
        is_htmx, "新建对账单", &claims, "sales",
        ReconciliationCreatePath::PATH, "销售管理", Some("新建对账单"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "create")]
pub async fn post_reconciliation_create(
    _path: ReconciliationCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReconciliationCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let reconciliation_svc = state.reconciliation_service();
    let id = reconciliation_svc
        .create(&service_ctx, &mut conn, form.customer_id, form.period)
        .await?;

    let detail_path = ReconciliationDetailPath { id };
    Ok((
        axum::http::StatusCode::OK,
        [("HX-Redirect", detail_path.to_string())],
        "",
    ))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_reconciliation_preview(
    ctx: RequestContext,
    Query(params): Query<PreviewQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let customer_id = match params.customer_id {
        Some(id) if id > 0 => id,
        _ => return Ok(Html(preview_empty("请选择客户").into_string())),
    };
    let period = match &params.period {
        Some(p) if !p.is_empty() => p.clone(),
        _ => return Ok(Html(preview_empty("请选择对账期间").into_string())),
    };

    let reconciliation_svc = state.reconciliation_service();
    let items = reconciliation_svc
        .preview(&service_ctx, &mut conn, customer_id, period)
        .await?;

    if items.is_empty() {
        return Ok(Html(preview_empty("该客户在所选期间内没有已发货数据").into_string()));
    }

    // Resolve product details
    let product_svc = state.product_service();
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_map: HashMap<i64, ProductInfo> = product_svc
        .get_by_ids(&service_ctx, &mut conn, product_ids)
        .await
        .map(|products| products.into_iter().map(|p| {
            (p.product_id, ProductInfo { code: p.product_code, name: p.pdt_name, unit: p.unit })
        }).collect())
        .unwrap_or_default();

    // Resolve order numbers
    let order_svc = state.sales_order_service();
    let order_ids: Vec<i64> = items.iter().map(|i| i.sales_order_id).collect::<std::collections::HashSet<_>>().into_iter().collect();
    let order_numbers: HashMap<i64, String> = {
        let mut map = HashMap::new();
        for &oid in &order_ids {
            if let Ok(order) = order_svc.find_by_id(&service_ctx, &mut conn, oid).await {
                map.insert(oid, order.doc_number);
            }
        }
        map
    };

    // Resolve shipping numbers
    let shipping_svc = state.shipping_service();
    let shipping_ids: Vec<i64> = items.iter().map(|i| i.shipping_request_id).collect::<std::collections::HashSet<_>>().into_iter().collect();
    let shipping_numbers: HashMap<i64, String> = {
        let mut map = HashMap::new();
        for &sid in &shipping_ids {
            if let Ok(shipping) = shipping_svc.find_by_id(&service_ctx, &mut conn, sid).await {
                map.insert(sid, shipping.doc_number);
            }
        }
        map
    };

    let content = preview_table(&items, &product_map, &order_numbers, &shipping_numbers);
    Ok(Html(content.into_string()))
}

// ── Components ──

fn reconciliation_create_page(
    customers: &[abt_core::master_data::customer::model::Customer],
) -> Markup {
    let periods = generate_periods();

    html! {
        div x-data="reconciliationForm()" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(ReconciliationListPath::PATH) {
                    (icon::chevron_left_icon("w-4 h-4"))
                    "返回对账单列表"
                }
                h1 class="page-title" { "新建对账单" }
            }

            form id="rec-create-form"
                  hx-post=(ReconciliationCreatePath::PATH)
                  hx-swap="none" {
                // ── Customer & Period ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "对账信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "客户名称" span style="color:var(--danger)" { "*" } }
                            select name="customer_id" x-model="customerId"
                                x-on:change="triggerPreview()" {
                                option value="" { "请选择客户" }
                                @for c in customers {
                                    option value=(c.id) { (c.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label { "对账期间" span style="color:var(--danger)" { "*" } }
                            select name="period" x-model="period"
                                x-on:change="triggerPreview()" {
                                option value="" { "请选择对账期间" }
                                @for (value, label) in &periods {
                                    option value=(value) { (label) }
                                }
                            }
                        }
                    }
                }

                // ── Preview Area ──
                div id="rec-preview-area"
                    class="data-card"
                    style="padding:0;overflow:hidden;margin-bottom:var(--space-4)"
                    hx-get=(ReconciliationPreviewPath::PATH)
                    hx-trigger="previewChanged from:div[x-data]"
                    hx-include="#rec-create-form select"
                    hx-target="this"
                    hx-swap="outerHTML" {
                    div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                        span class="form-section-title" style="margin:0;padding:0;border:none" { "对账明细预览" }
                        span style="font-size:var(--text-sm);color:var(--muted)" { "基于已发货数据自动聚合" }
                    }
                    div style="overflow-x:auto" {
                        table class="data-table" style="min-width:800px" {
                            thead {
                                tr {
                                    th style="width:36px;text-align:center" { "#" }
                                    th { "来源发货单" }
                                    th { "关联订单" }
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th style="width:56px" { "单位" }
                                    th style="width:80px;text-align:right" { "数量" }
                                    th style="width:100px;text-align:right" { "单价" }
                                    th style="width:100px;text-align:right" { "金额" }
                                }
                            }
                            tbody {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        (icon::clipboard_list_icon("w-5 h-5"))
                                        " 选择客户和对账期间后，将预览对账明细"
                                    }
                                }
                            }
                        }
                    }
                    div class="totals-bar" {
                        div class="totals-item" {
                            span class="totals-label" { "明细行数" }
                            span class="totals-value" { "0 行" }
                        }
                        div class="totals-item" {
                            span class="totals-label" { "总数量" }
                            span class="totals-value" { "0" }
                        }
                        div class="totals-item" {
                            span class="totals-label" { "总金额" }
                            span class="totals-value" { "¥ 0.00" }
                        }
                    }
                }

                // ── Remark ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "备注" }
                    textarea name="remark" placeholder="输入对账相关备注信息…"
                        style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(ReconciliationListPath::PATH) { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="submit" class="btn btn-primary"
                            x-bind:disabled="!customerId || !period" {
                            "创建对账单"
                        }
                    }
                }
            }

            script src="/reconciliation-create.js" {}
        }
    }
}

fn preview_empty(message: &str) -> Markup {
    html! {
        div id="rec-preview-area" class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
            div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                span class="form-section-title" style="margin:0;padding:0;border:none" { "对账明细预览" }
                span style="font-size:var(--text-sm);color:var(--muted)" { "基于已发货数据自动聚合" }
            }
            div style="overflow-x:auto" {
                table class="data-table" style="min-width:800px" {
                    thead {
                        tr {
                            th style="width:36px;text-align:center" { "#" }
                            th { "来源发货单" }
                            th { "关联订单" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th style="width:56px" { "单位" }
                            th style="width:80px;text-align:right" { "数量" }
                            th style="width:100px;text-align:right" { "单价" }
                            th style="width:100px;text-align:right" { "金额" }
                        }
                    }
                    tbody {
                        tr {
                            td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                (icon::clipboard_list_icon("w-5 h-5"))
                                " " (message)
                            }
                        }
                    }
                }
            }
            div class="totals-bar" {
                div class="totals-item" {
                    span class="totals-label" { "明细行数" }
                    span class="totals-value" { "0 行" }
                }
                div class="totals-item" {
                    span class="totals-label" { "总数量" }
                    span class="totals-value" { "0" }
                }
                div class="totals-item" {
                    span class="totals-label" { "总金额" }
                    span class="totals-value" { "¥ 0.00" }
                }
            }
        }
    }
}

fn preview_table(
    items: &[ReconciliationPreviewItem],
    product_map: &HashMap<i64, ProductInfo>,
    order_numbers: &HashMap<i64, String>,
    shipping_numbers: &HashMap<i64, String>,
) -> Markup {
    let total_amount: rust_decimal::Decimal = items.iter().map(|i| i.amount).sum();
    let total_qty: rust_decimal::Decimal = items.iter().map(|i| i.quantity).sum();
    let item_count = items.len();

    html! {
        div id="rec-preview-area" class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
            div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                span class="form-section-title" style="margin:0;padding:0;border:none" { "对账明细预览" }
                span style="font-size:var(--text-sm);color:var(--muted)" { "基于已发货数据自动聚合" }
            }
            div style="overflow-x:auto" {
                table class="data-table" style="min-width:800px" {
                    thead {
                        tr {
                            th style="width:36px;text-align:center" { "#" }
                            th { "来源发货单" }
                            th { "关联订单" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th style="width:56px" { "单位" }
                            th style="width:80px;text-align:right" { "数量" }
                            th style="width:100px;text-align:right" { "单价" }
                            th style="width:100px;text-align:right" { "金额" }
                        }
                    }
                    tbody {
                        @for (i, item) in items.iter().enumerate() {
                            @let product = product_map.get(&item.product_id);
                            @let product_code = product.map(|p| p.code.as_str()).unwrap_or("—");
                            @let product_name = product.map(|p| p.name.as_str()).unwrap_or("—");
                            @let unit = product.map(|p| p.unit.as_str()).unwrap_or("—");
                            @let order_num = order_numbers.get(&item.sales_order_id).map(|s| s.as_str()).unwrap_or("—");
                            @let shipping_num = shipping_numbers.get(&item.shipping_request_id).map(|s| s.as_str()).unwrap_or("—");
                            @let shipping_detail = ShippingDetailPath { id: item.shipping_request_id };
                            @let order_detail = OrderDetailPath { id: item.sales_order_id };

                            tr {
                                td class="line-num" { (i + 1) }
                                td {
                                    a href=(shipping_detail.to_string()) style="color:var(--info)" { (shipping_num) }
                                }
                                td {
                                    a href=(order_detail.to_string()) style="color:var(--info)" { (order_num) }
                                }
                                td class="mono" { (product_code) }
                                td { (product_name) }
                                td { (unit) }
                                td class="num-right" { (item.quantity) }
                                td class="num-right mono" { (format!("{:.2}", item.unit_price)) }
                                td class="num-right mono" { (format!("{:.2}", item.amount)) }
                            }
                        }
                    }
                }
            }
            div class="totals-bar" {
                div class="totals-item" {
                    span class="totals-label" { "明细行数" }
                    span class="totals-value" { (item_count) " 行" }
                }
                div class="totals-item" {
                    span class="totals-label" { "总数量" }
                    span class="totals-value" { (total_qty) }
                }
                div class="totals-item" {
                    span class="totals-label" { "总金额" }
                    span class="totals-value" style="font-weight:600" { "¥ " (format!("{:.2}", total_amount)) }
                }
            }
        }
    }
}

// ── Referenced paths from other route modules ──

use crate::routes::shipping::ShippingDetailPath;
use crate::routes::order::OrderDetailPath;
