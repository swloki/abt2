use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::{CustomerContact, CustomerQuery};
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::quotation::QuotationService;
use abt_core::sales::quotation::model::QuotationItem;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::types::PageParams;
use std::collections::HashMap;

use crate::components::customer_info::{customer_info_panel, CustomerContactsParams};
use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query Params ──

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct OrderCreateForm {
 pub customer_id: i64,
 pub contact_id: i64,
 pub payment_terms: Option<String>,
 pub delivery_terms: Option<String>,
 pub delivery_address: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
 pub profit_center_id: Option<String>,
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
 item_delivery_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrderCreateQueryParams {
 pub from_quotation: Option<i64>,
}

#[allow(dead_code)]
struct OrderPrefill {
 customer_id: i64,
 contact_id: i64,
 payment_terms: Option<String>,
 delivery_terms: Option<String>,
 remark: Option<String>,
 items: Vec<QuotationItem>,
 product_names: HashMap<i64, String>,
 product_codes: HashMap<i64, String>,
 contacts: Vec<CustomerContact>,
}

#[require_permission("SALES_ORDER", "create")]
pub async fn get_order_create(
 _path: OrderCreatePath,
 ctx: RequestContext,
 Query(params): Query<OrderCreateQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let customer_svc = state.customer_service();

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;
 use abt_core::master_data::profit_center::ProfitCenterService;
 let profit_centers = state.profit_center_service().list(&mut conn).await?;

 // ── Pre-fill from quotation if specified ──
 let mut prefill = None;
 if let Some(qid) = params.from_quotation {
 let q_svc = state.quotation_service();
 let product_svc = state.product_service();
 if let Ok(q) = q_svc.find_by_id(&service_ctx, &mut conn, qid).await {
 let q_items = q_svc.list_items(&service_ctx, &mut conn, qid).await.unwrap_or_default();
 let product_ids: Vec<i64> = q_items.iter().map(|i| i.product_id).collect();
 let products = if !product_ids.is_empty() {
 product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
 } else { vec![] };
 let p_names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
 let p_codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();

 let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, q.customer_id).await.unwrap_or_default();

 prefill = Some(OrderPrefill {
 customer_id: q.customer_id,
 contact_id: q.contact_id,
 payment_terms: Some(q.payment_terms.clone()),
 delivery_terms: Some(q.delivery_terms.clone()),
 remark: if q.remark.is_empty() { None } else { Some(q.remark.clone()) },
 items: q_items,
 product_names: p_names,
 product_codes: p_codes,
 contacts,
 });
 }
 }

 let content = order_create_page(&customers.items, &prefill, &profit_centers);
 let page_html = admin_page(
 is_htmx, "新建订单", &claims, "sales", OrderCreatePath::PATH, "销售管理", Some("新建订单"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

/// HTMX: fetch customer contacts → return full customer-info panel
#[require_permission("SALES_ORDER", "read")]
pub async fn get_customer_contacts(
 ctx: RequestContext,
 Query(params): Query<CustomerContactsParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let customer_svc = state.customer_service();

 let contacts = match params.customer_id {
 Some(cid) if cid > 0 => {
 customer_svc.list_contacts(&service_ctx, &mut conn, cid).await.unwrap_or_default()
 }
 _ => vec![],
 };

 let result = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 Ok(Html(customer_info_panel(&result.items, &contacts, params.customer_id, OrderCustomerContactsPath::PATH).into_string()))
}



#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 product_id: i64,
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("SALES_ORDER", "create")]
pub async fn get_order_item_row(
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
/// POST: create order from form submission (HTMX)
#[require_permission("SALES_ORDER", "create")]
pub async fn create_order(
 _path: OrderCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<OrderCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

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

 let items: Vec<CreateSalesOrderItemReq> = web_items.into_iter().map(|item| {
 CreateSalesOrderItemReq {
 product_id: item.product_id.parse().unwrap_or(0),
 description: item.description,
 quantity: item.quantity.parse().unwrap_or(rust_decimal::Decimal::ONE),
 unit: item.unit,
 unit_price: item.unit_price.parse().unwrap_or(rust_decimal::Decimal::ZERO),
 unit_cost: item.unit_cost.and_then(|s| s.parse().ok()),
 discount_rate: item.discount_rate.and_then(|s| s.parse().ok()),
 delivery_date: item.item_delivery_date.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
 }
 }).collect();

 let total: rust_decimal::Decimal = items.iter().map(|i| {
 let subtotal = i.quantity * i.unit_price;
 let discount = i.discount_rate.unwrap_or(rust_decimal::Decimal::ZERO) / rust_decimal::Decimal::ONE_HUNDRED;
 subtotal * (rust_decimal::Decimal::ONE - discount)
 }).sum();
 if total <= rust_decimal::Decimal::ZERO {
 return Err(DomainError::validation("订单总额不能为零，请填写产品单价").into());
 }

 let create_req = CreateSalesOrderReq {
 customer_id: form.customer_id,
 contact_id: form.contact_id,
 items,
 payment_terms: form.payment_terms,
 delivery_terms: form.delivery_terms,
 delivery_address: form.delivery_address,
 remark: form.remark,
 profit_center_id: form.profit_center_id.as_deref().and_then(|s| if s.is_empty() { None } else { s.parse().ok() }),
 };

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let id = svc.create(&service_ctx, &mut tx, create_req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = OrderDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components: Page ──

// ── Components: Page ──

#[allow(clippy::type_complexity)]
fn order_create_page(customers: &[abt_core::master_data::customer::model::Customer], prefill: &Option<OrderPrefill>, profit_centers: &[abt_core::master_data::profit_center::ProfitCenter]) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();

 // Pre-fill values
 let (sel_customer_id, sel_contacts, sel_payment, sel_delivery, sel_remark): (Option<i64>, &[CustomerContact], Option<&str>, Option<&str>, Option<&str>) = if let Some(p) = prefill {
 (Some(p.customer_id), &p.contacts, p.payment_terms.as_deref(), p.delivery_terms.as_deref(), p.remark.as_deref())
 } else {
 (None, &[], None, None, None)
 };

 html! {
    div id="order-app" class="p-6" {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(format!("{}?restore=true", OrderListPath::PATH))
            { (icon::arrow_left_icon("w-4 h-4")) "返回订单列表" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建订单" }
        }

        form id="order-form" hx-post=(OrderCreatePath::PATH) hx-swap="none" {
            input type="hidden" id="items-json" name="items_json" value="[]" {}
            // ── Customer Info (HTMX self-contained) ──
            ({
                customer_info_panel(
                    customers,
                    sel_contacts,
                    sel_customer_id,
                    OrderCustomerContactsPath::PATH,
                )
            })
            // ── Order Info ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "订单信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "订单日期"
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="date"
                            value=(today)
                            readonly {}
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "业务员"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="sales_rep"
                        {
                            option value="" { "当前用户" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "利润中心" }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="profit_center_id"
                        {
                            option value="" { "不指定" }
                            @for pc in profit_centers {
                                option value=(pc.id) { (pc.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "付款条款"
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="payment_terms"
                        {
                            option value="30天净额" selected[sel_payment == Some("30天净额")] {
                                "30天净额"
                            }
                            option value="60天净额" selected[sel_payment == Some("60天净额")] {
                                "60天净额"
                            }
                            option value="预付30%" selected[sel_payment == Some("预付30%")] {
                                "预付30%"
                            }
                            option value="货到付款" selected[sel_payment == Some("货到付款")] {
                                "货到付款"
                            }
                            option value="月结30天" selected[sel_payment == Some("月结30天")] {
                                "月结30天"
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "交货条款"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="delivery_terms"
                        {
                            option value="FOB 深圳" selected[sel_delivery == Some("FOB 深圳")] {
                                "FOB 深圳"
                            }
                            option value="FOB 广州" selected[sel_delivery == Some("FOB 广州")] {
                                "FOB 广州"
                            }
                            option value="CIF 目的港" selected[sel_delivery == Some("CIF 目的港")] {
                                "CIF 目的港"
                            }
                            option value="EXW 工厂交货" selected[sel_delivery == Some("EXW 工厂交货")] {
                                "EXW 工厂交货"
                            }
                        }
                    }
                    div class="form-field col-span-2" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "交货地址"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            name="delivery_address"
                            placeholder="默认取客户地址，可修改" {}
                    }
                }
            }
            // ── Line Items ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::package_icon("w-[18px] h-[18px]")) "产品明细" }
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th class="w-12" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th class="w-20" { "单位" }
                                th class="w-24" { "数量" }
                                th class="w-28" { "单价 (¥)" }
                                th class="w-20" { "折扣%" }
                                th class="w-32" { "小计 (¥)" }
                                th class="col-date" { "交货日期" }
                                th class="w-16" {}
                            }
                        }
                        tbody id="order-item-tbody" {
                            @if let Some(p) = prefill {
                                @for item in &p.items {
                                    (prefill_item_row(item, &p.product_names, &p.product_codes))
                                }
                            }
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
                div class="flex justify-end p-4 bg-surface border-t border-border-soft gap-8" {
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "合计金额" }
                        span class="text-lg font-bold text-fg" id="subtotal-value" { "¥ 0.00" }
                    }
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "折扣总额" }
                        span class="text-lg font-bold text-fg" id="discount-value" { "- ¥ 0.00" }
                    }
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "订单总额" }
                        span class="text-lg font-bold text-fg grand" id="grand-value" { "¥ 0.00" }
                    }
                }
            }
            // ── Remark ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::file_text_icon("w-[18px] h-[18px]")) "备注" }
                textarea
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] min-h-[72px] resize-y leading-1.5"
                    name="remark"
                    placeholder="输入订单相关备注信息…"
                {
                    @if let Some(r) = sel_remark { (r) }
                }
            }
            // ── Attachment ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::upload_icon("w-[18px] h-[18px]")) "附件" }
                div class="rounded p-8 text-center cursor-pointer" {
                    (icon::upload_icon("w-8 h-8"))
                    p class="upload-title" { "点击或拖拽文件到此处上传" }
                    p class="upload-hint" { "支持 PDF、Word、Excel、图片，单个文件不超过 10MB" }
                }
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", OrderListPath::PATH))
                { "取消" }
                div class="flex gap-3" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    { (icon::save_icon("w-4 h-4")) "保存草稿" }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        _="on click call salesOrderSubmit() then trigger submit on #order-form"
                    { (icon::send_icon("w-4 h-4")) "提交订单" }
                }
            }
        }
        // ── Product Selection Modal ──
        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "product-modal",
                OrderItemRowPath::PATH,
                "order-item-tbody",
            )
        })
    }
    // ── Pre-fill: recalculate totals after page load ──
    @if prefill.is_some() {
        ({
            maud::PreEscaped(
                r#"<script>document.addEventListener('DOMContentLoaded',function(){if(typeof salesOrderRecalcTotals==='function')salesOrderRecalcTotals()})</script>"#,
            )
        })
    }
}
}

fn prefill_item_row(item: &QuotationItem, names: &HashMap<i64, String>, codes: &HashMap<i64, String>) -> Markup {
 let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
 let discount = if item.discount_rate > rust_decimal::Decimal::ZERO {
 item.discount_rate.to_string()
 } else {
 String::new()
 };

 html! {
    tr oninput="salesOrderCalcRow(this)" {
        td class="text-muted text-xs text-center" {}
        td class="font-mono tabular-nums" { (product_code) }
        td { (product_name) }
        td {
            input
                class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm outline-none focus:border-accent"
                type="text"
                name="description"
                value=(item.description.as_str()) {}
        }
        td {
            input
                class="w-[56px] text-center px-2 py-[5px] text-[13px] border border-border rounded-sm bg-surface outline-none focus:border-accent"
                type="text"
                name="unit"
                readonly
                value=(item.unit.as_str()) {}
        }
        td {
            input
                class="w-[80px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent"
                type="number"
                step="any"
                name="quantity"
                value=(item.quantity) {}
        }
        td {
            input
                class="w-[100px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent"
                type="number"
                step="any"
                name="unit_price"
                value=(item.unit_price) {}
        }
        td {
            input
                class="w-[64px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent"
                type="number"
                step="any"
                name="discount_rate"
                value=(discount) {}
        }
        td class="text-right font-semibold text-fg whitespace-nowrap" { "—" }
        td {
            input
                class="w-[110px] px-1.5 py-[5px] text-xs border border-border rounded-sm outline-none focus:border-accent"
                type="date"
                name="item_delivery_date"
                value=(delivery) {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center"
                title="删除行"
                _="on click remove closest <tr/>"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(item.product_id) {}
    }
}
}

fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
    tr oninput="salesOrderCalcRow(this)" {
        td class="text-muted text-xs text-center" {}
        td class="font-mono tabular-nums" { (product.product_code) }
        td { (product.pdt_name) }
        td {
            input
                class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm outline-none focus:border-accent"
                type="text"
                name="description" {}
        }
        td {
            input
                class="w-[56px] text-center px-2 py-[5px] text-[13px] border border-border rounded-sm bg-surface outline-none focus:border-accent"
                type="text"
                name="unit"
                readonly
                value=(product.unit) {}
        }
        td {
            input
                class="w-[80px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent"
                type="number"
                step="any"
                name="quantity"
                placeholder="0" {}
        }
        td {
            input
                class="w-[100px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent"
                type="number"
                step="any"
                name="unit_price"
                placeholder="0.00" {}
        }
        td {
            input
                class="w-[64px] text-right px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm outline-none focus:border-accent"
                type="number"
                step="any"
                name="discount_rate" {}
        }
        td class="text-right font-semibold text-fg whitespace-nowrap" { "—" }
        td {
            input
                class="w-[110px] px-1.5 py-[5px] text-xs border border-border rounded-sm outline-none focus:border-accent"
                type="date"
                name="item_delivery_date" {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center"
                title="删除行"
                _="on click remove closest <tr/>"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(product.product_id) {}
    }
}
}
