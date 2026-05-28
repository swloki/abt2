use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;
use tower_sessions::Session;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::sales::quotation::QuotationService;
use abt_core::sales::quotation::model::*;
use abt_core::shared::types::{PageParams, PgExecutor, ServiceContext};

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::customer_info::{CustomerContactsParams, customer_info_panel};
use crate::components::icon;
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::quotation::{
    QuotationCreatePath, QuotationCustomerContactsPath, QuotationDetailPath, QuotationListPath,
    QuotationProductsPath,
};
use crate::state::AppState;

// ── Helpers ──

fn make_ctx<'a>(
    conn: &'a mut sqlx::postgres::PgConnection,
    operator_id: i64,
) -> ServiceContext<'a> {
    ServiceContext::new(conn as PgExecutor<'a>, operator_id)
}

async fn get_claims(session: &Session) -> abt_core::shared::identity::model::Claims {
    session
        .get(CURRENT_USER_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| abt_core::shared::identity::model::Claims {
            sub: 0,
            username: "未知用户".into(),
            display_name: "未知用户".into(),
            system_role: "user".into(),
            role_ids: vec![],
            role_codes: vec![],
            department_ids: vec![],
            iss: String::new(),
            exp: 0,
            iat: 0,
        })
}

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
    product_id: i64,
    description: Option<String>,
    quantity: String,
    unit: Option<String>,
    unit_price: String,
    unit_cost: Option<String>,
    discount_rate: Option<String>,
    delivery_date: Option<String>,
}

// ── Handlers ──

pub async fn get_quotation_create(
    _path: QuotationCreatePath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(&mut conn, claims.sub);
    let customers = customer_svc
        .list(
            ctx,
            CustomerQuery {
                name: None,
                status: None,
                category: None,
                owner_id: None,
            },
            PageParams::new(1, 200),
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let content = quotation_create_page(&customers.items);
    let page_html = admin_page(
        &headers,
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
pub async fn get_customer_contacts(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<CustomerContactsParams>,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let contacts = match params.customer_id {
        Some(cid) if cid > 0 => {
            let ctx = make_ctx(&mut conn, claims.sub);
            customer_svc
                .list_contacts(ctx, cid)
                .await
                .unwrap_or_default()
        }
        _ => vec![],
    };

    let ctx2 = make_ctx(&mut conn, claims.sub);
    let result = customer_svc
        .list(
            ctx2,
            CustomerQuery {
                name: None,
                status: None,
                category: None,
                owner_id: None,
            },
            PageParams::new(1, 200),
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

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
pub async fn get_products(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let svc = state.product_service();
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let filter = ProductQuery {
        name: params.name,
        code: params.code,
        status: None,
        owner_department_id: None,
    };
    let ctx = make_ctx(&mut conn, claims.sub);
    let result = svc
        .list(ctx, filter, PageParams::new(1, 20))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

/// POST: create quotation from form submission (HTMX)
pub async fn create_quotation(
    _path: QuotationCreatePath,
    State(state): State<AppState>,
    session: Session,
    axum::Form(form): axum::Form<QuotationCreateForm>,
) -> Result<impl IntoResponse, AppError> {
    let claims = get_claims(&session).await;
    let svc = state.quotation_service();
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
        .map_err(|e| AppError::Internal(format!("无效日期格式: {e}")))?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| AppError::BadRequest(format!("无效产品数据: {e}")))?;

    let items: Vec<CreateQuotationItemReq> = web_items
        .into_iter()
        .map(|item| CreateQuotationItemReq {
            product_id: item.product_id,
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
        .collect();

    let create_req = CreateQuotationReq {
        customer_id: form.customer_id,
        contact_id: form.contact_id,
        valid_until,
        items,
        payment_terms: form.payment_terms,
        delivery_terms: form.delivery_terms,
        remark: form.remark,
    };

    let mut tx: sqlx::Transaction<'_, sqlx::Postgres> =
        sqlx::Connection::begin(&mut *conn)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    let ctx = ServiceContext::new(&mut *tx, claims.sub);
    let id = svc.create(ctx, create_req).await?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

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
        div x-data="quotationForm()" {
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
                  hx-swap="none" {
                input type="hidden" name="items_json" x-model="itemsJson";

            // ── Customer Info (HTMX self-contained) ──
            (customer_info_panel(customers, &[], None, QuotationCustomerContactsPath::PATH))

            // ── Quote Info ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "报价信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "报价日期" }
                        input type="date" name="quotation_date" value=(today) disabled {}
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
                        x-on:click="productModalOpen = true" {
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
                        tbody {
                            template x-for="(item, idx) in items" {
                                tr {
                                    td class="line-num" x-text="idx + 1" {}
                                    td class="mono" x-text="item.product_code" {}
                                    td x-text="item.product_name" {}
                                    td { input class="form-input" type="text" x-model="item.description" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td { input class="form-input" type="text" x-model="item.unit" readonly style="width:56px;text-align:center;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--surface)" {} }
                                    td { input class="form-input num-input" type="number" x-model="item.quantity" min="1" step="1" placeholder="0" style="width:80px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td { input class="form-input num-input" type="number" x-model="item.unit_price" step="0.01" placeholder="0.00" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td { input class="form-input num-input" type="number" x-model="item.discount_rate" min="0" max="100" style="width:64px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td class="line-total" x-text="subtotal(idx) > 0 ? '¥ ' + subtotal(idx).toFixed(2) : '—'" style="text-align:right;font-family:var(--font-mono);font-weight:600;white-space:nowrap" {}
                                    td { button type="button" class="btn-remove-row" x-on:click="removeItem(idx)" title="删除行" {
                                        (icon::x_icon("w-3.5 h-3.5"))
                                    } }
                                }
                            }
                        }
                    }
                }
                div class="add-row-bar" {
                    button type="button" class="btn-add-row"
                        x-on:click="productModalOpen = true" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产品行"
                    }
                }
                div class="totals-bar" {
                    div class="totals-item" {
                        span class="totals-label" { "合计金额" }
                        span class="totals-value" x-text="'¥ ' + lineTotal.toFixed(2)" { "¥ 0.00" }
                    }
                    div class="totals-item" {
                        span class="totals-label" { "折扣总额" }
                        span class="totals-value" x-text="'- ¥ ' + discountTotal.toFixed(2)" { "- ¥ 0.00" }
                    }
                    div class="totals-item" {
                        span class="totals-label" { "报价总额" }
                        span class="totals-value grand" x-text="'¥ ' + grandTotal.toFixed(2)" { "¥ 0.00" }
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
            div class="modal-overlay"
                x-bind:class="{ 'is-open': productModalOpen }"
                x-on:click="productModalOpen = false" {
                div class="modal modal-lg" x-on:click="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "选择产品" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            x-on:click="productModalOpen = false" { "×" }
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
                                    onclick="document.querySelectorAll('.product-search-input').forEach(function(i){i.value=''})" {
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

            // ── Submit script ──
            script src="/quotation-create.js" {}
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
                    @let product_json = serde_json::json!({
                        "product_id": p.product_id,
                        "product_code": &p.product_code,
                        "product_name": &p.pdt_name,
                        "specification": &p.meta.specification,
                        "unit": &p.unit,
                    }).to_string();
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
                            data-product=(product_json)
                            x-on:click="addItem(JSON.parse($el.dataset.product))" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}
