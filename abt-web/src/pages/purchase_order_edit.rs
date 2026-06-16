use std::collections::HashMap;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::order::model::{
    CreateOrderItemRequest, PurchaseOrder, PurchaseOrderItem, UpdatePurchaseOrderRequest,
};
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::TaxRateService;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::shared::types::pagination::PageParams;
use abt_core::shared::types::DomainError;

use abt_macros::require_permission;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::RequestContext;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct POEditForm {
    pub supplier_id: i64,
    pub expected_delivery_date: Option<String>,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
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

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_edit(path: POEditPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        claims,
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let svc = state.purchase_order_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();

    let order = svc.get(&service_ctx, &mut conn, path.id).await?;

    if order.status != PurchaseOrderStatus::Draft {
        return Err(DomainError::business_rule("仅草稿状态的订单可以编辑").into());
    }

    let items = svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

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

    // Resolve product info for existing items
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_map: HashMap<i64, (String, String)> = if !product_ids.is_empty() {
        let products = product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .unwrap_or_default();
        products
            .into_iter()
            .map(|p| (p.product_id, (p.product_code, p.pdt_name)))
            .collect()
    } else {
        HashMap::new()
    };

    let tax_rates = state.tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let content = po_edit_page(&order, &items, &suppliers.items, &product_map, &tax_rates);
    let page_html = admin_page(
        is_htmx,
        "编辑采购订单",
        &claims,
        "purchase",
        &format!("/admin/purchase/orders/{}/edit", path.id),
        "采购管理",
        Some("编辑采购订单"),
        content,
        &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

/// POST: update purchase order
#[require_permission("PURCHASE_ORDER", "update")]
pub async fn update_po(
    path: POEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<POEditForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_order_service();
    let existing_order = svc.get(&service_ctx, &mut conn, path.id).await?;

    if form.supplier_id == 0 {
        return Err(DomainError::validation("请选择供应商").into());
    }

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

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个产品").into());
    }

    let items: Vec<CreateOrderItemRequest> = web_items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let item_expected_delivery_date = item
                .item_delivery_date
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

            let quantity: Decimal = item
                .quantity
                .parse()
                .map_err(|_| DomainError::validation(format!("第 {} 行无效数量", idx + 1)))?;
            let unit_price: Decimal = item
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
                    .unwrap_or(Decimal::ZERO),
                tax_rate_id: item.tax_rate_id.as_deref()
                    .and_then(|s| s.parse().ok())
                    .filter(|&v: &i64| v > 0),
            })
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    let req = UpdatePurchaseOrderRequest {
        supplier_id: form.supplier_id,
        expected_delivery_date,
        payment_terms: form.payment_terms,
        delivery_address: form.delivery_address,
        remark: form.remark.unwrap_or_default(),
        currency_code: existing_order.currency_code.clone(),
        currency_rate: existing_order.currency_rate,
        discount_amount: existing_order.discount_amount,
    };

    svc.update(&service_ctx, &mut conn, path.id, req, items)
        .await?;

    let redirect = PODetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn po_edit_page(
    order: &PurchaseOrder,
    items: &[PurchaseOrderItem],
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    product_map: &HashMap<i64, (String, String)>,
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
    let edit_path = format!("/admin/purchase/orders/{}/edit", order.id);
    let expected_delivery = order
        .expected_delivery_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let payment_terms = order.payment_terms.as_deref().unwrap_or("");
    let delivery_address = order.delivery_address.as_deref().unwrap_or("");
    let remark = order.remark.as_str();

    html! {
        div id="po-app" {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(PODetailPath { id: order.id }.to_string()) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回订单详情"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { "编辑采购订单 — " (order.doc_number) }
            }

            form id="po-form"
                  hx-post=(&edit_path)
                  hx-swap="none" {
                input type="hidden" id="items-json" name="items_json" value="[]";

            // ── Supplier Selection ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "供应商信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "供应商" span style="color:var(--danger)" { "*" } }
                        select name="supplier_id" required {
                            @for s in suppliers {
                                @if s.id == order.supplier_id {
                                    option value=(s.id) selected { (s.name) }
                                } @else {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                    }
                }
            }

            // ── Order Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "订单信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "订单日期" }
                        input type="date" value=(order.order_date.format("%Y-%m-%d").to_string()) readonly
                            style="background:var(--bg-muted)" {}
                    }
                    div class="form-field" {
                        label { "预期交货日期" }
                        input type="date" name="expected_delivery_date" value=(&expected_delivery) {}
                    }
                    div class="form-field" {
                        label { "付款条件" }
                        select name="payment_terms" {
                            option value="" selected[payment_terms.is_empty()] { "请选择付款条件" }
                            @for opt in ["30天净额", "60天净额", "预付30%", "货到付款", "月结30天"] {
                                @if opt == payment_terms {
                                    option value=(opt) selected { (opt) }
                                } @else {
                                    option value=(opt) { (opt) }
                                }
                            }
                        }
                    }
                    div class="form-field span-2" {
                        label { "交货地址" }
                        input type="text" name="delivery_address" placeholder="输入交货地址…" value=(delivery_address) {}
                    }
                    div class="form-field span-2" {
                        label { "备注" }
                        textarea name="remark" placeholder="输入订单相关备注信息…"
                            style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {
                            (remark)
                        }
                    }
                }
            }

            // ── Line Items ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" style="margin:0;padding:0;border:none" { "采购产品明细" }
                    button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm bg-accent text-accent-on border-none hover:bg-accent-hover [&_svg]:w-4 [&_svg]:h-4"
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
                        tbody id="po-item-tbody" {
                            @for item in items {
                                (existing_item_row(item, product_map, tax_rates))
                            }
                        }
                    }
                }
                div class="p-3 flex items-center gap-2" {
                    button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
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
                a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" href=(PODetailPath { id: order.id }.to_string()) { "取消" }
                button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" { "保存修改" }
            }
            // ── Form submit: collect items into JSON ──
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
            div class="fixed z-[1000] grid place-items-center opacity-0" id="product-modal"
                _="on click remove .is-open from #product-modal" {
                div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-lg" onclick="event.stopPropagation()" {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                        h2 { "选择产品" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            _="on click remove .is-open from #product-modal" { "×" }
                    }
                    div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" {
                        div class="flex gap-4 p-4 border-b" {
                            div class="flex-1 flex flex-col gap-[4px]" {
                                label class="text-[12px] font-medium text-fg-2" { "产品名称" }
                                input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name" placeholder="输入产品名称…"
                                    hx-get=(POProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="flex-1 flex flex-col gap-[4px]" {
                                label class="text-[12px] font-medium text-fg-2" { "产品编码" }
                                input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(POProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
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

/// Render an existing item row with pre-filled values (same structure as create page's item_row_fragment)
fn existing_item_row(
    item: &PurchaseOrderItem,
    product_map: &HashMap<i64, (String, String)>,
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
    let (code, name) = product_map
        .get(&item.product_id)
        .cloned()
        .unwrap_or_else(|| ("—".into(), "—".into()));
    let subtotal = item.quantity * item.unit_price;
    let delivery = item
        .expected_delivery_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let input_style = "width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)";

    html! {
        tr data-item-row="" {
            td class="text-text-muted text-xs text-center" { }
            td class="font-mono tabular-nums" { (code) }
            td { (name) }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="description" placeholder="—" value=(&item.description)
                style="width:190px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="1" min="0.01" name="quantity" data-field="qty" placeholder="0"
                value=(item.quantity.to_string()) style=(input_style) {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" min="0.01" name="unit_price" data-field="price" placeholder="0.00"
                value=(item.unit_price.to_string()) style=(input_style) {} }
            td class="line-subtotal font-mono tabular-nums" data-field="subtotal" style="text-align:right" { (subtotal.to_string()) }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="0.01" min="0" max="100" name="discount_pct" data-field="discount" value=(item.discount_pct.to_string()) placeholder="0" style=(input_style) {} }
            td {
                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="tax_rate_id" data-field="tax_rate_id"
                    style="width:110px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {
                    option value="" { "—" }
                    @for tr in tax_rates {
                        @if item.tax_rate_id == Some(tr.id) {
                            option value=(tr.id) data-rate=(tr.rate.to_string()) selected { (tr.name) }
                        } @else {
                            option value=(tr.id) data-rate=(tr.rate.to_string()) { (tr.name) }
                        }
                    }
                }
            }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="item_delivery_date" value=(&delivery)
                style="width:110px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="w-[28px] h-[28px] border-none text-text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
                _="on click remove closest <tr/> then call updatePurchaseSummary()" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(item.product_id) {}
        }
    }
}
