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
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(PODetailPath { id: order.id }.to_string())
            { (icon::arrow_left_icon("w-4 h-4")) "返回订单详情" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "编辑采购订单 — " (order.doc_number) }
        }

        form id="po-form" hx-post=(&edit_path) hx-swap="none" {
            input type="hidden" id="items-json" name="items_json" value="[]";
            // ── Supplier Selection ──
            div class="data-card" class="mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "供应商信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "供应商"
                            span class="text-danger" { "*" }
                        }
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
            div class="data-card" class="mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "订单信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "订单日期" }
                        input
                            type="date"
                            value=(order.order_date.format("%Y-%m-%d").to_string())
                            readonly
                            class="bg-surface" {}
                    }
                    div class="form-field" {
                        label { "预期交货日期" }
                        input type="date" name="expected_delivery_date" value=(&expected_delivery) {}
                    }
                    div class="form-field" {
                        label { "付款条件" }
                        select name="payment_terms" {
                            option value="" selected[payment_terms.is_empty()] { "请选择付款条件" }
                            @for opt in {
                                [
                                    "30天净额",
                                    "60天净额",
                                    "预付30%",
                                    "货到付款",
                                    "月结30天",
                                ]
                            } {
                                @if opt == payment_terms {
                                    option value=(opt) selected { (opt) }
                                } @else {
                                    option value=(opt) { (opt) }
                                }
                            }
                        }
                    }
                    div class="form-field col-span-2" {
                        label { "交货地址" }
                        input
                            type="text"
                            name="delivery_address"
                            placeholder="输入交货地址…"
                            value=(delivery_address) {}
                    }
                    div class="form-field col-span-2" {
                        label { "备注" }
                        textarea
                            name="remark"
                            placeholder="输入订单相关备注信息…"
                            class="w-full resize-y"
                            class="rounded-sm"
                            class="min-h-[80px] border border-border text-sm"
                            style="padding:8px 12px;font-family:inherit"
                        { (remark) }
                    }
                }
            }
            // ── Line Items ──
            div class="data-card" class="p-0 overflow-hidden mb-4" {
                div class="flex justify-between items-center" class="px-5 pt-5 pb-3" {
                    span
                        class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                        class="m-0 p-0 border-none"
                    { "采购产品明细" }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                        _="on click add .is-open to #product-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加产品" }
                }
                div class="overflow-x-auto" {
                    table class="data-table" class="min-w-[900px]" {
                        thead {
                            tr {
                                th class="w-9 text-center" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th class="w-[200px]" { "描述" }
                                th class="w-[100px] text-right" { "数量" }
                                th class="w-[120px] text-right" { "单价" }
                                th class="w-[110px] text-right" { "小计" }
                                th class="w-[80px] text-right" { "折扣%" }
                                th class="w-[150px]" { "税率" }
                                th class="w-[120px]" { "预期交货日期" }
                                th class="w-9" {}
                            }
                        }
                        tbody id="po-item-tbody" {
                            @for item in items { (existing_item_row(item, product_map, tax_rates)) }
                        }
                    }
                }
                div class="p-3 flex items-center gap-2" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
                        _="on click add .is-open to #product-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加产品行" }
                }
                div class="flex justify-end" class="p-4" class="border-t border-border" {
                    div class="flex" class="text-sm" class="gap-6" {
                        div {
                            "不含税: "
                            span id="sum-untaxed" class="font-semibold" { "0.00" }
                        }
                        div {
                            "税额: "
                            span id="sum-tax" class="font-semibold" { "0.00" }
                        }
                        div {
                            "含税总计: "
                            span id="sum-total" class="font-semibold" class="text-accent" { "0.00" }
                        }
                    }
                }
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(PODetailPath { id: order.id }.to_string())
                { "取消" }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { "保存修改" }
            }
            // ── Form submit: collect items into JSON ──
            script {
                ({
                    maud::PreEscaped(
                        "document.currentScript.parentElement.addEventListener('submit', function(ev){
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
 })",
                    )
                })
            }
        }

        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "product-modal",
                POItemRowPath::PATH,
                "po-item-tbody",
            )
        })
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
        td class="text-muted text-xs text-center" {}
        td class="font-mono tabular-nums" { (code) }
        td { (name) }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                type="text"
                name="description"
                placeholder="—"
                value=(&item.description)
                class="text-[13px]"
                class="rounded-sm"
                class="px-2 py-[5px] border border-border"
                style="width:190px" {}
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input"
                type="number"
                step="any"
                name="quantity"
                data-field="qty"
                placeholder="0"
                value=(item.quantity.normalize().to_string())
                style=(input_style) {}
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input"
                type="number"
                step="any"
                name="unit_price"
                data-field="price"
                placeholder="0.00"
                value=(item.unit_price.to_string())
                style=(input_style) {}
        }
        td class="line-subtotal font-mono tabular-nums" data-field="subtotal" class="text-right" {
            (subtotal.to_string())
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input"
                type="number"
                step="any"
                name="discount_pct"
                data-field="discount"
                value=(item.discount_pct.to_string())
                placeholder="0"
                style=(input_style) {}
        }
        td {
            select
                class="min-w-[150px] w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                name="tax_rate_id"
                data-field="tax_rate_id"
                class="text-[13px]"
                class="rounded-sm"
                class="px-2 py-[5px] border border-border"
            {
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
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                type="date"
                name="item_delivery_date"
                value=(&delivery)
                class="w-[110px] text-[13px]"
                class="rounded-sm"
                class="px-2 py-[5px] border border-border" {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center"
                title="删除行"
                _="on click remove closest <tr/> then call updatePurchaseSummary()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(item.product_id) {}
    }
}
}
