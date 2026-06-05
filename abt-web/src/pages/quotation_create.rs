use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::sales::quotation::QuotationService;
use abt_core::sales::quotation::model::*;
use abt_core::shared::types::PageParams;

use crate::components::customer_info::{CustomerContactsParams, customer_info_panel};
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::quotation::{
    QuotationCreatePath, QuotationCustomerContactsPath, QuotationDetailPath, QuotationItemRowPath,
    QuotationListPath, QuotationProductsPath,
};
use crate::utils::RequestContext;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
    pub name: Option<String>,
    pub code: Option<String>,
}

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct QuotationCreateForm {
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
    delivery_date: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "create")]
pub async fn get_quotation_create(
    _path: QuotationCreatePath,
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
    let customer_svc = state.customer_service();

    let customers = customer_svc
        .list(
            &service_ctx,
            &mut conn,
            CustomerQuery {
                name: None,
                status: None,
                category: None,
                owner_id: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let content = quotation_create_page(&customers.items);
    let page_html = admin_page(
        is_htmx,
        "新建报价单",
        &claims,
        "sales",
        QuotationCreatePath::PATH,
        "销售管理",
        Some("新建报价单"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: fetch customer contacts → return full customer-info panel
#[require_permission("SALES_ORDER", "read")]
pub async fn get_customer_contacts(
    ctx: RequestContext,
    Query(params): Query<CustomerContactsParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let customer_svc = state.customer_service();

    let contacts = match params.customer_id {
        Some(cid) if cid > 0 => customer_svc
            .list_contacts(&service_ctx, &mut conn, cid)
            .await
            .unwrap_or_default(),
        _ => vec![],
    };

    let result = customer_svc
        .list(
            &service_ctx,
            &mut conn,
            CustomerQuery {
                name: None,
                status: None,
                category: None,
                owner_id: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    Ok(Html(
        customer_info_panel(
            &result.items,
            &contacts,
            params.customer_id,
            QuotationCustomerContactsPath::PATH,
        )
        .into_string(),
    ))
}

/// HTMX: search products → return HTML fragment
#[require_permission("PRODUCT", "read")]
pub async fn get_products(
    ctx: RequestContext,
    Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.product_service();

    let filter = ProductQuery {
            name: params.name.filter(|s| !s.is_empty()),
            code: params.code.filter(|s| !s.is_empty()),
            status: None,
            owner_department_id: None,
            category_id: None,
        };
    let result = svc
        .list(&service_ctx, &mut conn, filter, PageParams::new(1, 20))
        .await?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    product_id: i64,
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("SALES_ORDER", "create")]
pub async fn get_quotation_item_row(
    ctx: RequestContext,
    Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.product_service();
    let product = svc
        .get(&service_ctx, &mut conn, params.product_id)
        .await?;
    Ok(Html(item_row_fragment(&product).into_string()))
}

/// POST: create quotation from form submission (HTMX)
#[require_permission("SALES_ORDER", "create")]
pub async fn create_quotation(
    _path: QuotationCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<QuotationCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.quotation_service();

    let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效日期格式: {e}")))?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

    let items: Vec<CreateQuotationItemReq> = web_items
        .into_iter()
        .map(|item| CreateQuotationItemReq {
            product_id: item.product_id.parse().unwrap_or(0),
            description: item.description,
            quantity: item.quantity.parse().unwrap_or(rust_decimal::Decimal::ONE),
            unit: item.unit,
            unit_price: item
                .unit_price
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            unit_cost: item.unit_cost.and_then(|s| s.parse().ok()),
            discount_rate: item.discount_rate.and_then(|s| s.parse().ok()),
            delivery_date: item
                .delivery_date
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        })
        .collect::<Vec<_>>();

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

    let create_req = CreateQuotationReq {
        customer_id: form.customer_id,
        contact_id: form.contact_id,
        valid_until,
        items,
        payment_terms: form.payment_terms,
        delivery_terms: form.delivery_terms,
        remark: form.remark,
    };

    let id = svc.create(&service_ctx, &mut conn, create_req).await?;

    let redirect = QuotationDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn quotation_create_page(customers: &[abt_core::master_data::customer::model::Customer]) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_valid = chrono::Local::now()
        .checked_add_days(chrono::Days::new(30))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    html! {
        div id="quotation-app" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(QuotationListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回报价单列表"
                }
                h1 class="page-title" { "新建报价单" }
            }
            form id="quotation-form"
                  hx-post=(QuotationCreatePath::PATH)
                  hx-swap="none"
                onsubmit="quotationSubmit(this)" {
                input type="hidden" id="items-json" name="items_json" value="[]";

            // ── Customer Info (HTMX self-contained) ──
            (customer_info_panel(customers, &[], None, QuotationCustomerContactsPath::PATH))

            // ── Quote Info ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "报价信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "报价日期" }
                        input type="date" name="quotation_date" value=(today) readonly {}
                    }
                    div class="form-field" {
                        label { "有效期至" span style="color:var(--danger)" { "*" } }
                        input type="date" name="valid_until" id="f-valid-until" value=(default_valid) {}
                    }
                    div class="form-field" {
                        label { "付款条款" }
                        select name="payment_terms" {
                            option value="30天净额" { "30天净额" }
                            option value="60天净额" { "60天净额" }
                            option value="预付30%" { "预付30%" }
                            option value="货到付款" { "货到付款" }
                            option value="月结30天" { "月结30天" }
                        }
                    }
                    div class="form-field" {
                        label { "交货条款" }
                        select name="delivery_terms" {
                            option value="FOB 深圳" { "FOB 深圳" }
                            option value="FOB 广州" { "FOB 广州" }
                            option value="CIF 目的港" { "CIF 目的港" }
                            option value="EXW 工厂交货" { "EXW 工厂交货" }
                        }
                    }
                }
            }

            // ── Line Items ──
            div class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="form-section-title" style="margin:0;padding:0;border:none" { "产品明细" }
                    button type="button" class="btn btn-sm btn-primary"
                        onclick="hsAdd(null,'#product-modal','is-open')" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产品"
                    }
                }
                div style="overflow-x:auto" {
                    table class="data-table" style="min-width:900px" {
                        thead {
                            tr {
                                th style="width:36px;text-align:center" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th style="width:56px" { "单位" }
                                th style="width:90px;text-align:right" { "数量" }
                                th style="width:110px;text-align:right" { "单价 (¥)" }
                                th style="width:76px;text-align:right" { "折扣%" }
                                th style="width:110px;text-align:right" { "小计 (¥)" }
                                th style="width:36px" { }
                            }
                        }
                        tbody id="quotation-item-tbody" { }
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
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "备注" }
                textarea name="remark" placeholder="输入报价相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
            }

            // ── Action Bar ──
            div class="create-action-bar" {
                a class="btn btn-default" href=(QuotationListPath::PATH) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="submit" class="btn btn-primary" {
                        "提交报价"
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
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            onclick="hsRemove(null,'#product-modal','is-open')" { "×" }
                    }
                    div class="modal-body" style="padding:0" {
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
                            div id="product-search-results" style="max-height:320px;overflow-y:auto"
                            hx-get=(QuotationProductsPath::PATH)
                            hx-trigger="intersect once"
                            hx-swap="innerHTML" {
                            div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                                "加载中…"
                            }
                        }
                    }
                }
            }

        }
    }
}

/// Product search results fragment
fn product_list_fragment(products: &[abt_core::master_data::product::model::Product]) -> Markup {
    html! {
        @if products.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                (icon::package_icon("w-8 h-8"))
                p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的产品" }
            }
        } @else {
            div class="product-select-list" {
                @for p in products {
                    div class="product-select-item" {
                        div class="product-select-info" {
                            div class="product-select-name" { (p.pdt_name) }
                            div class="product-select-meta" {
                                span class="product-select-code" { (p.product_code) }
                                span class="product-select-sep" { "·" }
                                span { (p.meta.specification) }
                                span class="product-select-sep" { "·" }
                                span { (p.unit) }
                            }
                        }
                        button type="button" class="btn btn-sm btn-primary"
                            hx-get=(format!("{}?product_id={}", QuotationItemRowPath::PATH, p.product_id))
                            hx-target="#quotation-item-tbody"
                            hx-swap="beforeend"
                            hx-on::after-request="hsRemove(null,'#product-modal','is-open')" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}

fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
    html! {
        tr oninput="quotationCalcRow(this)" {
            td class="line-num" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td { input class="form-input" type="text" name="description" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input" type="text" name="unit" readonly value=(product.unit) style="width:56px;text-align:center;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--surface)" {} }
            td { input class="form-input num-input" type="number" min="1" step="1" name="quantity" placeholder="0" style="width:80px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input num-input" type="number" step="0.01" name="unit_price" placeholder="0.00" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input num-input" type="number" min="0" max="100" name="discount_rate" style="width:64px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td class="line-total" style="text-align:right;font-family:var(--font-mono);font-weight:600;white-space:nowrap" { "—" }
            td { button type="button" class="btn-remove-row" title="删除行"
                onclick="hsRemoveClosestEl(this,'tr')" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
