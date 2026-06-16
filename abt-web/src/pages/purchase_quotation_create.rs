use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::quotation::model::*;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_quotation::{
    PQCreatePath, PQDetailPath, PQItemRowPath, PQListPath, PQProductsPath,
    PQSupplierContactsPath,
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PQCreateForm {
    pub supplier_id: i64,
    pub quotation_date: String,
    pub valid_from: String,
    pub valid_until: String,
    pub currency: Option<String>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub buyer_id: Option<i64>,
    pub remark: Option<String>,
    pub items_json: String,
    pub action: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
    product_id: String,
    unit_price: String,
    min_order_qty: Option<String>,
    lead_time_days: Option<String>,
    currency: Option<String>,
    is_preferred: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_create(
    _path: PQCreatePath,
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
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = pq_create_page(&suppliers.items, &users);
    let page_html = admin_page(
        is_htmx,
        "新建采购报价",
        &claims,
        "purchase",
        PQCreatePath::PATH,
        "采购管理",
        Some("新建采购报价"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search products → return HTML fragment
#[require_permission("PRODUCT", "read")]
pub async fn get_pq_products(
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

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_item_row(
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

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    product_id: i64,
}

/// HTMX: return supplier contact info fragment (contact, phone, address)
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_supplier_contacts(
    ctx: RequestContext,
    Query(params): Query<SupplierContactParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let supplier_svc = state.supplier_service();

    let contacts = if params.supplier_id > 0 {
        supplier_svc
            .list_contacts(&service_ctx, &mut conn, params.supplier_id)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Find primary contact, or fall back to first
    let primary = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());

    let contact_name = primary.map(|c| c.name.as_str()).unwrap_or("");
    let contact_phone = primary
        .and_then(|c| c.phone.as_deref())
        .unwrap_or("");

    Ok(Html(
        supplier_contact_fields_fragment(contact_name, contact_phone).into_string(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct SupplierContactParams {
    pub supplier_id: i64,
}

/// POST: create purchase quotation from form submission (HTMX)
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn create_pq(
    _path: PQCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PQCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_quotation_service();

    let quotation_date = chrono::NaiveDate::parse_from_str(&form.quotation_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效报价日期格式: {e}")))?;
    let valid_from = chrono::NaiveDate::parse_from_str(&form.valid_from, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效生效日期格式: {e}")))?;
    let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效失效日期格式: {e}")))?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

    let items: Vec<CreateQuotationItemRequest> = web_items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| CreateQuotationItemRequest {
            product_id: item.product_id.parse().unwrap_or(0),
            line_no: (idx as i32) + 1,
            unit_price: item
                .unit_price
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            min_order_qty: item.min_order_qty.and_then(|s| s.parse().ok()),
            lead_time_days: item.lead_time_days.and_then(|s| s.parse().ok()),
            currency: item.currency.unwrap_or_else(|| "CNY".to_string()),
            is_preferred: item.is_preferred.is_some(),
        })
        .collect();

    let create_req = CreatePurchaseQuotationRequest {
        supplier_id: form.supplier_id,
        quotation_date,
        valid_from,
        valid_until,
        remark: form.remark.unwrap_or_default(),
        items,
    };

    let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

    let redirect = PQDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pq_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    users: &[abt_core::shared::identity::model::User],
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_valid = chrono::Local::now()
        .checked_add_days(chrono::Days::new(30))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    html! {
        div id="pq-app" {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PQListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回采购报价列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购报价" }
            }

            form id="pq-form"
                  hx-post=(PQCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" id="items-json" name="items_json" value="[]";
                input type="hidden" id="form-action" name="action" value="submit";

            // ── Supplier Selection ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "供应商信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "供应商" span style="color:var(--danger)" { "*" } }
                        select name="supplier_id" required
                            hx-get=(PQSupplierContactsPath::PATH)
                            hx-trigger="change"
                            hx-target="#supplier-contact-fields"
                            hx-swap="innerHTML"
                            hx-vals="js:{supplier_id: this.value}" {
                            option value="" disabled selected { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" id="supplier-contact-fields" {
                    div class="form-field" {
                        label { "联系人" }
                        input type="text" readonly placeholder="—" style="background:var(--bg-muted)" {}
                    }
                    div class="form-field" {
                        label { "联系电话" }
                        input type="text" readonly placeholder="—" style="background:var(--bg-muted)" {}
                    }
                }
            }

            // ── Quote Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "报价信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "报价日期" }
                        input type="date" name="quotation_date" value=(today) readonly {}
                    }
                    div class="form-field" {
                        label { "生效日期" span style="color:var(--danger)" { "*" } }
                        input type="date" name="valid_from" id="f-valid-from" value=(today) {}
                    }
                    div class="form-field" {
                        label { "失效日期" span style="color:var(--danger)" { "*" } }
                        input type="date" name="valid_until" id="f-valid-until" value=(default_valid) {}
                    }
                    div class="form-field" {
                        label { "币种" }
                        select name="currency" {
                            option value="CNY" selected { "CNY (人民币)" }
                            option value="USD" { "USD (美元)" }
                            option value="EUR" { "EUR (欧元)" }
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
                }
            }

            // ── Line Items ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" style="margin:0;padding:0;border:none" { "报价产品明细" }
                    button type="button" class="btn btn-sm btn-primary"
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
                                th style="width:120px;text-align:right" { "单价" }
                                th style="width:100px;text-align:right" { "最小订购量" }
                                th style="width:90px;text-align:right" { "交货天数" }
                                th style="width:80px;text-align:center" { "币种" }
                                th style="width:56px;text-align:center" { "首选" }
                                th style="width:36px" { }
                            }
                        }
                        tbody id="pq-item-tbody" { }
                    }
                }
                div class="add-row-bar" {
                    button type="button" class="btn-add-row"
                        _="on click add .is-open to #product-modal" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产品行"
                    }
                }
            }

            // ── Remark ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "备注" }
                textarea name="remark" placeholder="输入报价相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
            }

            // ── Action Bar ──
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="btn btn-default" href=(format!("{}?restore=true", PQListPath::PATH)) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="button" class="btn btn-default"
                        _="on click set #form-action's value to 'draft' then call document.querySelector('#pq-form').requestSubmit()" {
                        "保存草稿"
                    }
                    button type="button" class="btn btn-primary" {
                        "提交报价"
                        (maud::PreEscaped(r#"<script>document.currentScript.parentElement.addEventListener('click', function() {
    var items = [];
    document.querySelectorAll('#pq-item-tbody tr').forEach(function(row) {
        var vals = {};
        row.querySelectorAll('input,select').forEach(function(el) {
            if (el.name && el.name.startsWith('item_')) vals[el.name.replace('item_','')] = el.value;
        });
        items.push(vals);
    });
    if (items.length === 0) {
        show_error_toast('请至少添加一个报价产品明细');
        return;
    }
    document.querySelector('#items-json').value = JSON.stringify(items);
    document.querySelector('#pq-form').requestSubmit();
})</script>"#))
                    }
                }
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
                                    hx-get=(PQProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="product-search-field" {
                                label class="product-search-label" { "产品编码" }
                                input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(PQProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                                button type="button" class="product-search-clear"
                                    hx-get=(PQProductsPath::PATH)
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    _="on click set (.product-search-input)'s value to '' then trigger keyup on .product-search-input" {
                                    "清除"
                                }
                            }
                            div id="product-search-results" style="max-height:320px;overflow-y:auto"
                            hx-get=(PQProductsPath::PATH)
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

/// Fragment returned by HTMX for supplier contact fields
fn supplier_contact_fields_fragment(contact_name: &str, contact_phone: &str) -> Markup {
    html! {
        div class="form-field" {
            label { "联系人" }
            input type="text" readonly value=(contact_name) placeholder="—" style="background:var(--bg-muted)" {}
        }
        div class="form-field" {
            label { "联系电话" }
            input type="text" readonly value=(contact_phone) placeholder="—" style="background:var(--bg-muted)" {}
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
                            hx-get=(format!("{}?product_id={}", PQItemRowPath::PATH, p.product_id))
                            hx-target="#pq-item-tbody"
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

fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
    html! {
        tr {
            td class="line-num" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" placeholder="0.00" style="width:110px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" name="item_unit_price" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0" placeholder="—" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" name="item_min_order_qty" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0" placeholder="—" style="width:80px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" name="item_lead_time_days" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" style="width:70px;text-align:center;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" name="item_currency" value="CNY" {} }
            td style="text-align:center" { input type="checkbox" name="item_is_preferred" style="width:16px;height:16px;cursor:pointer;accent-color:var(--primary)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                _="on click remove closest <tr/>" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="item_product_id" value=(product.product_id) {}
        }
    }
}
