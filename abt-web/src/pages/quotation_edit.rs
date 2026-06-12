use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::{CustomerContact, CustomerQuery};
use abt_core::master_data::product::ProductService;
use abt_core::sales::quotation::model::*;
use abt_core::sales::quotation::QuotationService;
use abt_core::shared::types::PageParams;

use crate::components::customer_info::customer_info_panel;
use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::quotation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct QuotationEditForm {
    pub customer_id: i64,
    pub contact_id: i64,
    pub valid_until: String,
    pub payment_terms: Option<String>,
    pub delivery_terms: Option<String>,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
    product_id: String,
    description: Option<String>,
    quantity: String,
    unit: Option<String>,
    unit_price: String,
    unit_cost: Option<String>,
    discount_rate: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_quotation_edit(
    path: EditQuotationFormPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();

    let quotation = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await?;

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, quotation.customer_id).await.unwrap_or_default();

    // Resolve product codes for items
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_codes: std::collections::HashMap<i64, (String, String)> = if !product_ids.is_empty() {
        let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
        products.into_iter().map(|p| (p.product_id, (p.product_code, p.pdt_name))).collect()
    } else {
        std::collections::HashMap::new()
    };

    let content = quotation_edit_page(&quotation, &items, &customers.items, &contacts, &product_codes);
    let page_html = admin_page(
        is_htmx, "编辑报价单", &claims, "sales",
        &format!("{}/{}", QuotationListPath::PATH, path.id),
        "销售管理", Some("编辑报价单"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

/// POST: update quotation
#[require_permission("SALES_ORDER", "update")]
pub async fn update_quotation(
    path: UpdateQuotationPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<QuotationEditForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    if form.customer_id == 0 {
        return Err(DomainError::validation("请选择客户").into());
    }
    if form.contact_id == 0 {
        return Err(DomainError::validation("请选择联系人").into());
    }

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个产品").into());
    }

    let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
        .map_err(|_| DomainError::validation("无效的有效期日期"))?;

    let items: Vec<CreateQuotationItemReq> = web_items.into_iter().map(|item| {
        CreateQuotationItemReq {
            product_id: item.product_id.parse().unwrap_or(0),
            description: item.description,
            quantity: item.quantity.parse().unwrap_or(rust_decimal::Decimal::ONE),
            unit: item.unit,
            unit_price: item.unit_price.parse().unwrap_or(rust_decimal::Decimal::ZERO),
            unit_cost: item.unit_cost.and_then(|s| s.parse().ok()),
            discount_rate: item.discount_rate.and_then(|s| s.parse().ok()),
            delivery_date: None,
        }
    }).collect::<Vec<_>>();

    let has_zero_price = items.iter().any(|i| i.unit_price <= rust_decimal::Decimal::ZERO);
    if has_zero_price {
        return Err(DomainError::validation("产品单价不能为 0，请检查所有产品的单价").into());
    }

    let total: rust_decimal::Decimal = items.iter().map(|i| {
        let subtotal = i.quantity * i.unit_price;
        let discount = i.discount_rate.unwrap_or(rust_decimal::Decimal::ZERO) / rust_decimal::Decimal::ONE_HUNDRED;
        subtotal * (rust_decimal::Decimal::ONE - discount)
    }).sum();
    if total <= rust_decimal::Decimal::ZERO {
        return Err(DomainError::validation("报价总额不能为零，请检查产品数量和单价").into());
    }

    let req = UpdateQuotationReq {
        customer_id: Some(form.customer_id),
        contact_id: Some(form.contact_id),
        sales_rep_id: None,
        valid_until: Some(valid_until),
        payment_terms: form.payment_terms,
        delivery_terms: form.delivery_terms,
        remark: form.remark,
        items: Some(items),
    };

    svc.update(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = QuotationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn quotation_edit_page(
    quotation: &Quotation,
    items: &[QuotationItem],
    customers: &[abt_core::master_data::customer::model::Customer],
    contacts: &[CustomerContact],
    product_codes: &std::collections::HashMap<i64, (String, String)>,
) -> Markup {
    let detail_path = QuotationDetailPath { id: quotation.id };
    let update_path = UpdateQuotationPath { id: quotation.id };

    let pt = &quotation.payment_terms;
    let dt = &quotation.delivery_terms;
    let rm = &quotation.remark;

    html! {
        div id="quotation-app" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(detail_path.to_string()) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回报价单详情"
                }
                h1 class="page-title" { "编辑报价单 " (quotation.doc_number) }
            }

            form id="quotation-form"
                  hx-post=(update_path.to_string())
                  hx-swap="none"
                  onsubmit="lineItemCalc('#quotation-item-tbody').collectItems()" {
                input type="hidden" id="items-json" name="items_json" value="[]";

            // ── Customer Info ──
            (customer_info_panel(customers, contacts, Some(quotation.customer_id), QuotationCustomerContactsPath::PATH))

            // ── Quote Info ──
            div class="form-section-card" {
                div class="form-section-title" { "报价信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "报价日期" }
                        input class="form-input" type="date" value=(quotation.quotation_date.format("%Y-%m-%d")) readonly {}
                    }
                    div class="form-field" {
                        label { "有效期至" span class="text-danger" { "*" } }
                        input class="form-input" type="date" name="valid_until" id="f-valid-until" value=(quotation.valid_until.format("%Y-%m-%d")) {}
                    }
                    div class="form-field" {
                        label { "付款条款" }
                        select class="form-select" name="payment_terms" {
                            option value="30天净额" selected[*pt == "30天净额"] { "30天净额" }
                            option value="60天净额" selected[*pt == "60天净额"] { "60天净额" }
                            option value="预付30%" selected[*pt == "预付30%"] { "预付30%" }
                            option value="货到付款" selected[*pt == "货到付款"] { "货到付款" }
                            option value="月结30天" selected[*pt == "月结30天"] { "月结30天" }
                        }
                    }
                    div class="form-field" {
                        label { "交货条款" }
                        select class="form-select" name="delivery_terms" {
                            option value="FOB 深圳" selected[*dt == "FOB 深圳"] { "FOB 深圳" }
                            option value="FOB 广州" selected[*dt == "FOB 广州"] { "FOB 广州" }
                            option value="CIF 目的港" selected[*dt == "CIF 目的港"] { "CIF 目的港" }
                            option value="EXW 工厂交货" selected[*dt == "EXW 工厂交货"] { "EXW 工厂交货" }
                        }
                    }
                }
            }

            // ── Line Items ──
            div class="form-section-card flush mb-4" {
                div class="flush-header" {
                    span class="form-section-title" { "产品明细" }
                    button type="button" class="btn btn-sm btn-primary"
                        onclick="hsAdd(null,'#product-modal','is-open')" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产品"
                    }
                }
                div class="flush-scroll" {
                    table class="line-items-table" {
                        thead {
                            tr {
                                th style="width:36px" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th style="width:56px" { "单位" }
                                th style="width:90px" { "数量" }
                                th style="width:110px" { "单价 (¥)" }
                                th style="width:76px" { "折扣%" }
                                th style="width:110px" { "小计 (¥)" }
                                th style="width:36px" { }
                            }
                        }
                        tbody id="quotation-item-tbody" {
                            @for item in items {
                                @let (code, name) = product_codes.get(&item.product_id).cloned().unwrap_or_default();
                                tr oninput="lineItemCalc('#quotation-item-tbody').calcRow(this)" {
                                    td class="line-num" { }
                                    td class="mono" { (code) }
                                    td { (name) }
                                    td { input class="li-input" type="text" name="description" value=(&item.description) {} }
                                    td { input class="li-input-center" type="text" name="unit" readonly value=(&item.unit) {} }
                                    td { input class="li-input-num" type="number" min="1" step="1" name="quantity" value=(item.quantity.to_string()) placeholder="0" style="width:80px" {} }
                                    td { input class="li-input-price" type="number" step="any" name="unit_price" value=(item.unit_price.to_string()) placeholder="0.00" style="width:100px" {} }
                                    td { input class="li-input-disc" type="number" min="0" max="100" name="discount_rate" value=(item.discount_rate.to_string()) style="width:64px" {} }
                                    td class="line-total" { "—" }
                                    td { button type="button" class="btn-remove-row" title="删除行"
                                        onclick="hsRemoveClosestEl(this,'tr')" {
                                        (icon::x_icon("w-3.5 h-3.5"))
                                    } }
                                    input type="hidden" name="product_id" value=(item.product_id) {}
                                }
                            }
                        }
                    }
                }
                div class="add-row-bar" {
                    button type="button" class="btn-add-row"
                        onclick="hsAdd(null,'#product-modal','is-open')" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产品行"
                    }
                }
                div class="totals-bar" {
                    div class="totals-item" {
                        span class="totals-label" { "合计金额" }
                        span class="totals-value" id="subtotal-value" { "¥ 0.00" }
                    }
                    div class="totals-item" {
                        span class="totals-label" { "折扣总额" }
                        span class="totals-value" id="discount-value" { "- ¥ 0.00" }
                    }
                    div class="totals-item" {
                        span class="totals-label" { "报价总额" }
                        span class="totals-value grand" id="grand-value" { "¥ 0.00" }
                    }
                }
            }

            // ── Remark ──
            div class="form-section-card" {
                div class="form-section-title" { "备注" }
                textarea class="form-textarea" name="remark" placeholder="输入报价相关备注信息…" { (rm) }
            }

            // ── Action Bar ──
            div class="create-action-bar" {
                a class="btn btn-default" href=(detail_path.to_string()) { "取消" }
                div class="flex gap-3" {
                    button type="submit" class="btn btn-primary" {
                        "保存修改"
                    }
                }
            }
            }

            // ── Product Selection Modal ──
            div class="modal-overlay" id="product-modal"
                onclick="hsRemove(null,'#product-modal','is-open')" {
                div class="modal modal-lg" onclick="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "选择产品" }
                        button class="modal-close-plain"
                            onclick="hsRemove(null,'#product-modal','is-open')" { "×" }
                    }
                    div class="modal-body p-0" {
                        div class="product-search-bar" {
                            div class="product-search-field" {
                                label class="product-search-label" { "产品名称" }
                                input class="product-search-input" type="text" name="name" placeholder="输入产品名称…"
                                    hx-get=(QuotationProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="product-search-field" {
                                label class="product-search-label" { "产品编码" }
                                input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(QuotationProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            button type="button" class="product-search-clear"
                                hx-get=(QuotationProductsPath::PATH)
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                onclick="hsSetAndTrigger('.product-search-input','','keyup')" {
                                "清除"
                            }
                        }
                        div id="product-search-results" class="product-search-scroll"
                        hx-get=(QuotationProductsPath::PATH)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML" {
                            div class="flex-center" style="padding:var(--space-8)" {
                                "加载中…"
                            }
                        }
                    }
                }
            }

            (maud::PreEscaped(r#"<script>document.addEventListener('DOMContentLoaded',function(){lineItemCalc('#quotation-item-tbody').recalcTotals()})</script>"#))
        }
    }
}
