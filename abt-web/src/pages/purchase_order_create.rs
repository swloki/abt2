use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::enums::PurchaseQuotationStatus;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::*;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::TaxRateService;
use abt_core::purchase::quotation::model::PurchaseQuotationQuery;
use abt_core::shared::identity::UserService;
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

#[derive(Debug, Deserialize)]
pub struct SupplierDetailParams {
    pub supplier_id: i64,
}

// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct POCreateForm {
    pub supplier_id: i64,
    pub order_date: String,
    pub expected_delivery_date: Option<String>,
    pub payment_terms: Option<String>,
    pub currency: Option<String>,
    pub delivery_address: Option<String>,
    pub related_quotation_id: Option<String>,
    pub buyer_id: Option<String>,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
    product_id: String,
    description: Option<String>,
    quantity: String,
    unit_price: String,
    item_delivery_date: Option<String>,
    discount_pct: Option<String>,
    tax_rate_id: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_create(
    _path: POCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let supplier_svc = state.supplier_service();
    let user_svc = state.user_service();
    let pq_svc = state.purchase_quotation_service();

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

    let users = user_svc
        .list_users(&service_ctx, &mut conn, 1, 200)
        .await?;

    let quotations = pq_svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseQuotationQuery {
                supplier_id: None,
                status: Some(PurchaseQuotationStatus::Active),
                quotation_date_start: None,
                quotation_date_end: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let tax_rates = state.tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let content = po_create_page(&suppliers.items, &users.items, &quotations.items, &tax_rates);
    let page_html = admin_page(
        is_htmx,
        "新建采购订单",
        &claims,
        "purchase",
        POCreatePath::PATH,
        "采购管理",
        Some("新建采购订单"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: return supplier detail fragment (contact/phone/address/info bar)
#[require_permission("SUPPLIER", "read")]
pub async fn get_po_supplier_detail(
    ctx: RequestContext,
    Query(params): Query<SupplierDetailParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.supplier_service();

    let supplier = svc.get(&service_ctx, &mut conn, params.supplier_id).await?;
    let contacts = svc
        .list_contacts(&service_ctx, &mut conn, params.supplier_id)
        .await
        .unwrap_or_default();

    let primary = contacts.iter().find(|c| c.is_primary);
    let contact_name = primary
        .map(|c| c.name.as_str())
        .unwrap_or("—");
    let contact_phone = primary
        .and_then(|c| c.phone.as_deref())
        .unwrap_or("—");

    // Compute cooperation years from created_at
    let coop_years = {
        let created = supplier.created_at;
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(created);
        diff.num_days() / 365
    };

    Ok(Html(
        supplier_detail_fragment(contact_name, contact_phone, coop_years).into_string(),
    ))
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

/// HTMX/JS: return active tax rates as JSON
#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_tax_rates(ctx: RequestContext) -> Result<axum::Json<serde_json::Value>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let rates = state.tax_rate_service()
        .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();
    let json: Vec<serde_json::Value> = rates.iter().map(|r| serde_json::json!({
 "id": r.id, "code": r.code, "name": r.name, "rate": r.rate.to_string()
    })).collect();
    Ok(axum::Json(serde_json::Value::Array(json)))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    product_id: i64,
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_item_row(
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
    let tax_rates = state.tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    Ok(Html(item_row_fragment(&product, &tax_rates).into_string()))
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
                .item_delivery_date
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

            let quantity: rust_decimal::Decimal = item
                .quantity
                .parse()
                .map_err(|_| DomainError::validation(format!("第 {} 行无效数量", idx + 1)))?;
            let unit_price: rust_decimal::Decimal = item
                .unit_price
                .parse()
                .map_err(|_| DomainError::validation(format!("第 {} 行无效单价", idx + 1)))?;

            Ok(CreateOrderItemRequest {
                product_id: item.product_id.parse().unwrap_or(0),
                line_no: (idx as i32) + 1,
                description: item.description.unwrap_or_default(),
                quantity,
                unit_price,
                quotation_item_id: None,
                expected_delivery_date: item_expected_delivery_date,
                discount_pct: item.discount_pct.as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(rust_decimal::Decimal::ZERO),
                tax_rate_id: item.tax_rate_id.as_deref()
                    .and_then(|s| s.parse().ok())
                    .filter(|&v: &i64| v > 0),
            })
        })
        .collect::<Result<Vec<_>, DomainError>>()?;

    let create_req = CreatePurchaseOrderRequest {
        supplier_id: form.supplier_id,
        order_date,
        expected_delivery_date,
        payment_terms: form.payment_terms,
        delivery_address: form.delivery_address,
        remark: form.remark.unwrap_or_default(),
        currency_code: form.currency.unwrap_or_else(|| String::from("CNY")),
        currency_rate: rust_decimal::Decimal::ONE,
        discount_amount: rust_decimal::Decimal::ZERO,
        items,
    };

    let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

    let redirect = PODetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn po_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    users: &[abt_core::shared::identity::model::User],
    quotations: &[abt_core::purchase::quotation::model::PurchaseQuotation],
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_delivery = chrono::Local::now()
        .checked_add_days(chrono::Days::new(15))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    html! {
        div id="po-app" {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", POListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回采购订单列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购订单" }
            }

            form id="po-form"
                  hx-post=(POCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" id="items-json" name="items_json" value="[]";

            // ── Supplier Selection ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "供应商信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "供应商" span style="color:var(--danger)" { "*" } }
                        select name="supplier_id" required
                            hx-get=(POSupplierDetailPath::PATH)
                            hx-trigger="change"
                            hx-target="#supplier-detail"
                            hx-swap="innerHTML"
                            hx-include="this" {
                            option value="" disabled selected { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "联系人" }
                        input type="text" id="supplier-contact" readonly placeholder="自动填充" style="background:var(--bg-muted)" {}
                    }
                    div class="form-field" {
                        label { "联系电话" }
                        input type="text" id="supplier-phone" readonly placeholder="自动填充" style="background:var(--bg-muted)" {}
                    }
                    div class="form-field span-2" {
                        label { "供应商地址" }
                        input type="text" id="supplier-address" readonly placeholder="自动填充" style="background:var(--bg-muted)" {}
                    }
                }
                // ── Supplier Info Bar ──
                div id="supplier-detail" style="margin-top:var(--space-3)" { }
            }

            // ── Order Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "订单信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "订单日期" }
                        input type="date" name="order_date" value=(today) readonly {}
                    }
                    div class="form-field" {
                        label { "预期交货日期" }
                        input type="date" name="expected_delivery_date" value=(default_delivery) {}
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
                        label { "币种" }
                        select name="currency" {
                            option value="CNY" selected { "CNY" }
                            option value="USD" { "USD" }
                            option value="EUR" { "EUR" }
                        }
                    }
                    div class="form-field span-2" {
                        label { "交货地址" }
                        input type="text" name="delivery_address" placeholder="输入交货地址…" {}
                    }
                    div class="form-field" {
                        label { "关联报价" }
                        select name="related_quotation_id" {
                            option value="" { "请选择采购报价" }
                            @for q in quotations {
                                option value=(q.id) { (q.doc_number) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "采购员" }
                        select name="buyer_id" {
                            option value="" { "请选择采购员" }
                            @for u in users {
                                @if u.is_active {
                                    option value=(u.user_id) { (u.display_name.as_deref().unwrap_or(&u.username)) }
                                }
                            }
                        }
                    }
                    div class="form-field span-2" {
                        label { "备注" }
                        textarea name="remark" placeholder="输入订单相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                    }
                }
            }

            // ── Line Items ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" style="margin:0;padding:0;border:none" { "采购产品明细" }
                    button type="button" class="btn btn-sm bg-accent text-accent-on border-none hover:bg-accent-hover"
                        _="on click add .is-open to #product-modal" {
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
                                th style="width:80px;text-align:right" { "折扣%" }
                                th style="width:120px" { "税率" }
                                th style="width:120px" { "预期交货日期" }
                                th style="width:36px" { }
                            }
                        }
                        tbody id="po-item-tbody" { }
                    }
                }
                div class="add-row-bar" {
                    button type="button" class="btn-add-row"
                        _="on click add .is-open to #product-modal" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产品行"
                    }
                }
                div style="display:flex;justify-content:flex-end;padding:var(--space-4);border-top:1px solid var(--border)" {
                    div style="display:flex;gap:var(--space-6);font-size:var(--text-sm)" {
                        div { "不含税: " span id="sum-untaxed" style="font-weight:600" { "0.00" } }
                        div { "税额: " span id="sum-tax" style="font-weight:600" { "0.00" } }
                        div { "含税总计: " span id="sum-total" style="font-weight:600;color:var(--primary)" { "0.00" } }
                    }
                }
            }

            // ── Action Bar ──
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="btn bg-white text-fg border border-border hover:bg-surface" href=(format!("{}?restore=true", POListPath::PATH)) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="button" class="btn bg-white text-fg border border-border hover:bg-surface" { "保存草稿" }
                    button type="submit" class="btn bg-accent text-accent-on border-none hover:bg-accent-hover" {
                        "提交订单"
                    }
                }
            }
            script {
                (maud::PreEscaped("document.currentScript.parentElement.addEventListener('submit', function(ev){
                    var errors=[];
                    document.querySelectorAll('#po-item-tbody tr').forEach(function(row, i){
                        var q=parseFloat(row.querySelector('[name=quantity]').value)||0;
                        var p=parseFloat(row.querySelector('[name=unit_price]').value)||0;
                        if(q<=0) errors.push('第'+(i+1)+'行数量必须大于0');
                        if(p<=0) errors.push('第'+(i+1)+'行单价必须大于0');
                    });
                    if(errors.length>0){ alert(errors.join('\\n')); ev.preventDefault(); return; }
                    var items=[];
                    document.querySelectorAll('#po-item-tbody tr').forEach(function(row){
                        var obj={};
                        row.querySelectorAll('input,select,textarea').forEach(function(el){
                            if(el.name && !obj[el.name]) obj[el.name]=el.value;
                        });
                        items.push(obj);
                    });
                    document.querySelector('#items-json').value=JSON.stringify(items);
                })"))
            }
            }

            // ── Product Selection Modal ──
            div class="modal-overlay" id="product-modal"
                _="on click remove .is-open from #product-modal" {
                div class="modal modal-lg" onclick="event.stopPropagation()" {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                        h2 { "选择产品" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            _="on click remove .is-open from #product-modal" { "×" }
                    }
                    div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" {
                        div class="product-search-bar" {
                            div class="product-search-field" {
                                label class="product-search-label" { "产品名称" }
                                input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name" placeholder="输入产品名称…"
                                    hx-get=(POProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="product-search-field" {
                                label class="product-search-label" { "产品编码" }
                                input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(POProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                                button type="button" class="product-search-clear"
                                    hx-get=(POProductsPath::PATH)
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    _="on click set (.product-search-input)'s value to '' then trigger keyup on .product-search-input" {
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

        }
    }
}

/// Supplier detail fragment returned by HTMX on supplier select change
fn supplier_detail_fragment(contact_name: &str, contact_phone: &str, coop_years: i64) -> Markup {
    html! {
        div class="supplier-info-bar" style="display:flex;gap:var(--space-6);padding:var(--space-3) var(--space-4);background:var(--bg-muted);border-radius:var(--radius-sm);font-size:var(--text-sm);color:var(--text-secondary)" {
            span { "联系人: " strong { (contact_name) } }
            span { "电话: " strong { (contact_phone) } }
            span { "地址: " strong { "—" } }
            span { "合作年限: " strong { (coop_years) " 年" } }
        }
        script {
            (maud::PreEscaped(format!("document.querySelector('#supplier-contact').value = '{}';", contact_name.replace('\'', "\\'"))))
            (maud::PreEscaped(format!("document.querySelector('#supplier-phone').value = '{}';", contact_phone.replace('\'', "\\'"))))
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
                        button type="button" class="btn btn-sm bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-get=(format!("{}?product_id={}", POItemRowPath::PATH, p.product_id))
                            hx-target="#po-item-tbody"
                            hx-swap="beforeend"
                            _="on 'htmx:afterRequest' remove .is-open from #product-modal" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}

fn item_row_fragment(
    product: &abt_core::master_data::product::model::Product,
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
    let input_style = "width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)";
    html! {
        tr data-item-row="" {
            td class="line-num" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="description" placeholder="—" style="width:190px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0.01" name="quantity" data-field="qty" placeholder="0" style=(input_style) {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" min="0.01" name="unit_price" data-field="price" placeholder="0.00" style=(input_style) {} }
            td class="line-subtotal mono" data-field="subtotal" style="text-align:right" { "0.00" }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="0.01" min="0" max="100" name="discount_pct" data-field="discount" value="0" placeholder="0" style=(input_style) {} }
            td {
                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="tax_rate_id" data-field="tax_rate_id"
                    style="width:110px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {
                    option value="" { "—" }
                    @for tr in tax_rates {
                        option value=(tr.id) data-rate=(tr.rate.to_string()) { (tr.name) }
                    }
                }
            }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="item_delivery_date" style="width:110px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                _="on click remove closest <tr/> then call updatePurchaseSummary()" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
