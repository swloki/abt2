use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::*;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
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
pub struct POCreateForm {
    pub supplier_id: i64,
    pub order_date: String,
    pub expected_delivery_date: Option<String>,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
    product_id: i64,
    description: Option<String>,
    quantity: String,
    unit_price: String,
    expected_delivery_date: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_create(
    _path: POCreatePath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let supplier_svc = state.supplier_service();

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

    let content = po_create_page(&suppliers.items);
    let page_html = admin_page(
        &headers,
        "新建采购订单",
        &claims,
        "purchase",
        POCreatePath::PATH,
        "采购管理",
        Some("新建采购订单"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search products → return HTML fragment
#[require_permission("PRODUCT", "read")]
pub async fn get_po_products(
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
        name: params.name,
        code: params.code,
        status: None,
        owner_department_id: None,
    };
    let result = svc
        .list(&service_ctx, &mut conn, filter, PageParams::new(1, 20))
        .await?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

/// POST: create purchase order from form submission (HTMX)
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn create_po(
    _path: POCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<POCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_order_service();

    let order_date = chrono::NaiveDate::parse_from_str(&form.order_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效订单日期格式: {e}")))?;

    let expected_delivery_date = form
        .expected_delivery_date
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| DomainError::validation(format!("无效预期交货日期格式: {e}")))
        })
        .transpose()?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

    let items: Vec<CreateOrderItemRequest> = web_items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let item_expected_delivery_date = item
                .expected_delivery_date
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

            CreateOrderItemRequest {
                product_id: item.product_id,
                line_no: (idx as i32) + 1,
                description: item.description.unwrap_or_default(),
                quantity: item
                    .quantity
                    .parse()
                    .unwrap_or(rust_decimal::Decimal::ZERO),
                unit_price: item
                    .unit_price
                    .parse()
                    .unwrap_or(rust_decimal::Decimal::ZERO),
                quotation_item_id: None,
                expected_delivery_date: item_expected_delivery_date,
            }
        })
        .collect();

    let create_req = CreatePurchaseOrderRequest {
        supplier_id: form.supplier_id,
        order_date,
        expected_delivery_date,
        payment_terms: form.payment_terms,
        delivery_address: form.delivery_address,
        remark: form.remark.unwrap_or_default(),
        items,
    };

    let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

    let redirect = PODetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn po_create_page(suppliers: &[abt_core::master_data::supplier::model::Supplier]) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    html! {
        div x-data="purchaseOrderForm()" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(POListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回采购订单列表"
                }
                h1 class="page-title" { "新建采购订单" }
            }

            form id="po-form"
                  hx-post=(POCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="items_json" x-model="itemsJson";

            // ── Supplier Selection ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "供应商信息" }
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
                }
            }

            // ── Order Info ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "订单信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "订单日期" }
                        input type="date" name="order_date" value=(today) disabled {}
                    }
                    div class="form-field" {
                        label { "预期交货日期" }
                        input type="date" name="expected_delivery_date" {}
                    }
                    div class="form-field" {
                        label { "付款条件" }
                        select name="payment_terms" {
                            option value="" { "请选择付款条件" }
                            option value="30天净额" { "30天净额" }
                            option value="60天净额" { "60天净额" }
                            option value="预付30%" { "预付30%" }
                            option value="货到付款" { "货到付款" }
                            option value="月结30天" { "月结30天" }
                        }
                    }
                    div class="form-field" {
                        label { "交货地址" }
                        input type="text" name="delivery_address" placeholder="输入交货地址…" {}
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
                                th style="width:200px" { "描述" }
                                th style="width:100px;text-align:right" { "数量" }
                                th style="width:120px;text-align:right" { "单价" }
                                th style="width:110px;text-align:right" { "小计" }
                                th style="width:120px" { "预期交货日期" }
                                th style="width:36px" { }
                            }
                        }
                        tbody {
                            template x-for="(item, idx) in items" {
                                tr {
                                    td class="line-num" x-text="idx + 1" {}
                                    td class="mono" x-text="item.product_code" {}
                                    td x-text="item.product_name" {}
                                    td { input class="form-input" type="text" x-model="item.description" placeholder="—" style="width:190px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td { input class="form-input num-input" type="number" x-model="item.quantity" step="1" min="0" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td { input class="form-input num-input" type="number" x-model="item.unit_price" step="0.01" placeholder="0.00" style="width:110px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                    td class="mono" style="text-align:right" x-text="subtotal(idx).toFixed(2)" {}
                                    td { input class="form-input" type="date" x-model="item.expected_delivery_date" style="width:110px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
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
            }

            // ── Remark ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "备注" }
                textarea name="remark" placeholder="输入订单相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
            }

            // ── Action Bar ──
            div class="create-action-bar" {
                a class="btn btn-default" href=(POListPath::PATH) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="submit" class="btn btn-primary" {
                        "提交订单"
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
                                    hx-get=(POProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="product-search-field" {
                                label class="product-search-label" { "产品编码" }
                                input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(POProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                                button type="button" class="product-search-clear"
                                    hx-get=(POProductsPath::PATH)
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    onclick="document.querySelectorAll('.product-search-input').forEach(function(i){i.value=''})" {
                                    "清除"
                                }
                            }
                            div id="product-search-results" style="max-height:320px;overflow-y:auto"
                            hx-get=(POProductsPath::PATH)
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
            script src="/purchase-order-create.js" {}
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
