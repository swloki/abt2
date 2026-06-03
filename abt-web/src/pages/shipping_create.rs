use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::*;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::model::*;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::PageParams;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::shipping::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn order_status_text(s: SalesOrderStatus) -> &'static str {
    match s {
        SalesOrderStatus::Draft => "草稿",
        SalesOrderStatus::Confirmed => "已确认",
        SalesOrderStatus::InProduction => "生产中",
        SalesOrderStatus::PartiallyShipped => "部分发货",
        SalesOrderStatus::Shipped => "已发货",
        SalesOrderStatus::Completed => "已完成",
        SalesOrderStatus::Cancelled => "已取消",
    }
}

// ── Data Structs ──

#[derive(Debug)]
struct OrderItemRow {
    order_id: i64,
    order_item_id: i64,
    product_id: i64,
    product_code: String,
    product_name: String,
    specification: Option<String>,
    unit: Option<String>,
    ordered_qty: rust_decimal::Decimal,
    shipped_qty: rust_decimal::Decimal,
}

// ── Form & Query Structs ──

#[derive(Debug, Deserialize)]
pub struct ShippingCreateForm {
    pub customer_id: i64,
    pub order_id: i64,
    pub expected_ship_date: Option<String>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ShippingItemWeb {
    order_item_id: i64,
    warehouse_id: i64,
    requested_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct CustomerContactsQuery {
    pub customer_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OrderSearchQuery {
    pub customer_id: Option<i64>,
    pub keyword: Option<String>,
}

// ── Handlers ──

#[require_permission("SHIPPING", "create")]
pub async fn get_shipping_create(
    _path: ShippingCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let customer_svc = state.customer_service();
    let warehouse_svc = state.warehouse_service();
    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 100)
        .await?;

    let content = shipping_create_page(&customers.items, &warehouses.items);
    let page_html = admin_page(
        is_htmx, "新建发货申请", &claims, "sales",
        ShippingCreatePath::PATH, "销售管理", Some("新建发货申请"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SHIPPING", "create")]
pub async fn post_shipping_create(
    _path: ShippingCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ShippingCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { claims: _, mut conn, state, service_ctx, .. } = ctx;

    let svc = state.shipping_service();

    if form.customer_id == 0 {
        return Err(DomainError::validation("请选择客户").into());
    }
    if form.order_id == 0 {
        return Err(DomainError::validation("请选择来源订单").into());
    }

    let web_items: Vec<ShippingItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个发货产品").into());
    }

    let items: Vec<CreateShippingItemReq> = web_items.into_iter().map(|item| {
        CreateShippingItemReq {
            order_item_id: item.order_item_id,
            warehouse_id: item.warehouse_id,
            requested_qty: item.requested_qty.parse().unwrap_or(rust_decimal::Decimal::ONE),
        }
    }).collect();

    let expected_ship_date = form.expected_ship_date
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let shipping_address = form.shipping_address.filter(|s| !s.is_empty());

    let req = CreateFromOrderReq {
        order_id: form.order_id,
        expected_ship_date,
        shipping_address,
        items,
    };

    let id = svc.create_from_order(&service_ctx, &mut conn, req).await?;

    let carrier = form.carrier.filter(|s| !s.is_empty());
    let remark = form.remark.filter(|s| !s.is_empty());
    if carrier.is_some() || remark.is_some() {
        svc.update(&service_ctx, &mut conn, id, UpdateShippingReq {
            carrier,
            remark,
            ..Default::default()
        }).await?;
    }

    let redirect = ShippingDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "read")]
pub async fn get_customer_contacts(
    ctx: RequestContext,
    Query(params): Query<CustomerContactsQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let customer_svc = state.customer_service();

    let (contacts, addresses) = match params.customer_id {
        Some(cid) if cid > 0 => {
            let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, cid).await.unwrap_or_default();
            let addresses = customer_svc.list_addresses(&service_ctx, &mut conn, cid).await.unwrap_or_default();
            (contacts, addresses)
        }
        _ => (vec![], vec![]),
    };

    let primary_contact = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());
    let contact_name = primary_contact.map(|c| c.name.as_str()).unwrap_or("");
    let contact_phone = primary_contact.and_then(|c| c.phone.as_deref()).unwrap_or("");

    let default_addr = addresses.iter()
        .find(|a| a.address_type == "shipping")
        .or_else(|| addresses.first());
    let shipping_address = default_addr.map(|a| {
        let mut parts = vec![a.province.clone(), a.city.clone()];
        if let Some(ref d) = a.district { parts.push(d.clone()); }
        parts.push(a.detail.clone());
        parts.join("")
    }).unwrap_or_default();

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    Ok(Html(customer_info_card(&customers.items, params.customer_id, contact_name, contact_phone, &shipping_address).into_string()))
}

#[require_permission("SHIPPING", "read")]
pub async fn get_order_search(
    ctx: RequestContext,
    Query(params): Query<OrderSearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let order_svc = state.sales_order_service();

    let customer_id = match params.customer_id {
        Some(id) if id > 0 => id,
        _ => return Ok(Html(order_search_empty().into_string())),
    };
    let filter = SalesOrderQuery {
        customer_id: Some(customer_id),
        keyword: params.keyword.clone(),
        ..Default::default()
    };
    let result = order_svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 10))
        .await?;

    if result.items.is_empty() {
        return Ok(Html(order_search_empty().into_string()));
    }

    // Collect all order items and product ids across orders
    let order_svc_inner = state.sales_order_service();
    let product_svc = state.product_service();

    let mut all_items: Vec<(i64, abt_core::sales::sales_order::model::SalesOrderItem)> = Vec::new();
    let mut all_product_ids: Vec<i64> = Vec::new();
    for order in &result.items {
        if let Ok(items) = order_svc_inner.list_items(&service_ctx, &mut conn, order.id).await {
            for item in &items {
                all_product_ids.push(item.product_id);
            }
            for item in items {
                all_items.push((order.id, item));
            }
        }
    }

    // Fetch product details for all product_ids
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = if all_product_ids.is_empty() {
        HashMap::new()
    } else {
        product_svc.get_by_ids(&service_ctx, &mut conn, all_product_ids)
            .await
            .map(|ps| ps.into_iter().map(|p| (p.product_id, p)).collect())
            .unwrap_or_default()
    };

    let item_rows: Vec<OrderItemRow> = all_items.into_iter().map(|(order_id, item)| {
        let product = product_map.get(&item.product_id);
        OrderItemRow {
            order_id,
            order_item_id: item.id,
            product_id: item.product_id,
            product_code: product.map(|p| p.product_code.clone()).unwrap_or_default(),
            product_name: product.map(|p| p.pdt_name.clone()).unwrap_or_default(),
            specification: product.map(|p| Some(p.meta.specification.clone())).unwrap_or(None),
            unit: product.map(|p| Some(p.unit.clone())).unwrap_or(None),
            ordered_qty: item.quantity,
            shipped_qty: item.shipped_qty,
        }
    }).collect();

    let mut items_map: HashMap<i64, Vec<&OrderItemRow>> = HashMap::new();
    for item in &item_rows {
        items_map.entry(item.order_id).or_default().push(item);
    }

    Ok(Html(order_search_results(&result.items, &items_map).into_string()))
}

// ── Components ──

fn shipping_create_page(
    customers: &[abt_core::master_data::customer::model::Customer],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    let warehouses_json = serde_json::to_string(
        &warehouses.iter().map(|w| serde_json::json!({
            "id": w.id,
            "name": &w.name,
        })).collect::<Vec<_>>()
    ).unwrap_or_default();

    html! {
        div x-data=(format!("shippingForm({warehouses_json})")) {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(ShippingListPath::PATH) {
                    (icon::chevron_left_icon("w-4 h-4"))
                    "返回发货申请列表"
                }
                h1 class="page-title" { "新建发货申请" }
            }

            form id="shipping-form"
                  hx-post=(ShippingCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="items_json" x-model="itemsJson";
                input type="hidden" name="order_id" x-model="selectedOrderId";

                // ── Customer Info ──
                (customer_info_card(customers, None, "", "", ""))

                // ── Order Picker ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "来源订单" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "选择订单" span style="color:var(--danger)" { "*" } }
                            div style="display:flex;gap:var(--space-2)" {
                                input type="text" readonly
                                    x-model="selectedOrderNumber"
                                    placeholder="点击选择来源订单"
                                    x-bind:disabled="!customerId"
                                    x-on:click="customerId && (orderModalOpen = true)"
                                    style="flex:1;cursor:pointer" {}
                                button type="button" class="btn btn-sm btn-default"
                                    x-show="selectedOrderNumber"
                                    x-on:click="clearOrder()" title="清除" {
                                    (icon::x_icon("w-3.5 h-3.5"))
                                }
                                button type="button" class="btn btn-sm btn-primary"
                                    x-bind:disabled="!customerId"
                                    x-on:click="orderModalOpen = true" {
                                    "选择订单"
                                }
                            }
                        }
                    }
                }

                // ── Shipping Info ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "发货信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "预计发货日期" }
                            input type="date" name="expected_ship_date" {}
                        }
                        div class="form-field" {
                            label { "承运商" }
                            select name="carrier" {
                                option value="" { "请选择承运商" }
                                option value="顺丰速运" { "顺丰速运" }
                                option value="中通快递" { "中通快递" }
                                option value="圆通速递" { "圆通速递" }
                                option value="韵达快递" { "韵达快递" }
                                option value="申通快递" { "申通快递" }
                                option value="京东物流" { "京东物流" }
                                option value="德邦物流" { "德邦物流" }
                                option value="自提" { "自提" }
                                option value="其他" { "其他" }
                            }
                        }
                    }
                }

                // ── Line Items ──
                div class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                    div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                        span class="form-section-title" style="margin:0;padding:0;border:none" { "发货产品明细" }
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
                                    th style="width:80px;text-align:right" { "订单数量" }
                                    th style="width:80px;text-align:right" { "已发货" }
                                    th style="width:90px;text-align:right" { "发货数量" }
                                    th style="width:140px" { "发货仓库" }
                                    th style="width:36px" { }
                                }
                            }
                            tbody {
                                template x-for="(item, idx) in items" {
                                    tr {
                                        td class="line-num" x-text="idx + 1" {}
                                        td class="mono" x-text="item.product_code" {}
                                        td x-text="item.product_name" {}
                                        td x-text="item.specification" {}
                                        td x-text="item.unit" {}
                                        td class="num-right" x-text="item.ordered_qty" {}
                                        td class="num-right" x-text="item.shipped_qty" {}
                                        td {
                                            input type="number" x-model="item.ship_qty" min="0" step="1"
                                                style="width:80px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {}
                                        }
                                        td {
                                            select x-model="item.warehouse_id"
                                                style="width:130px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {
                                                option value="" { "选择仓库" }
                                                template x-for="w in warehouses" {
                                                    option x-bind:value="w.id" x-text="w.name" {}
                                                }
                                            }
                                        }
                                        td {
                                            button type="button" class="btn-remove-row" x-on:click="removeItem(idx)" title="删除行" {
                                                (icon::x_icon("w-3.5 h-3.5"))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div class="totals-bar" {
                        div class="totals-item" {
                            span class="totals-label" { "产品数" }
                            span class="totals-value" x-text="totalItems + ' 项'" { "0 项" }
                        }
                        div class="totals-item" {
                            span class="totals-label" { "发货总数" }
                            span class="totals-value" x-text="totalQty.toFixed(0)" { "0" }
                        }
                    }
                }

                // ── Remark ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "备注" }
                    textarea name="remark" placeholder="输入发货相关备注信息…"
                        style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(ShippingListPath::PATH) { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="submit" class="btn btn-primary" {
                            "提交发货申请"
                        }
                    }
                }
            }

            // ── Order Picker Modal ──
            div class="modal-overlay"
                x-bind:class="{ 'is-open': orderModalOpen }"
                x-on:click="orderModalOpen = false" {
                div class="modal modal-lg" x-on:click="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "选择来源订单" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            x-on:click="orderModalOpen = false" { "×" }
                    }
                    div class="modal-body" style="padding:0" {
                        div class="product-search-bar" {
                            input type="hidden" name="customer_id" x-model="customerId" {}
                            div class="product-search-field" {
                                label class="product-search-label" { "搜索订单" }
                                input class="product-search-input" type="text" name="keyword" placeholder="输入订单号…"
                                    hx-get=(ShippingOrderSearchPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#shipping-order-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar input" {}
                            }
                        }
                        div id="shipping-order-results" style="max-height:360px;overflow-y:auto"
                            hx-get=(ShippingOrderSearchPath::PATH)
                            hx-trigger="intersect once"
                            hx-include=".product-search-bar input"
                            hx-swap="innerHTML" {
                            div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                                "加载中…"
                            }
                        }
                    }
                }
            }

            // ── Submit script ──
            script src="/shipping-create.js" {}
        }
    }
}

fn customer_info_card(
    customers: &[abt_core::master_data::customer::model::Customer],
    selected_customer_id: Option<i64>,
    contact_name: &str,
    contact_phone: &str,
    shipping_address: &str,
) -> Markup {
    let selected = selected_customer_id.map(|id| id.to_string()).unwrap_or_default();

    html! {
        div class="data-card" style="margin-bottom:var(--space-4)" {
            div class="form-section-title" { "客户信息" }
            div class="form-grid" {
                div class="form-field" {
                    label { "客户名称" span style="color:var(--danger)" { "*" } }
                    select name="customer_id" x-model="customerId"
                        hx-get=(ShippingCustomerContactsPath::PATH)
                        hx-trigger="change"
                        hx-target="closest .data-card"
                        hx-swap="outerHTML"
                        hx-include="this" {
                        option value="" { "请选择客户" }
                        @for c in customers {
                            option value=(c.id) selected[selected == c.id.to_string()] { (c.name) }
                        }
                    }
                }
                div class="form-field" {
                    label { "联系人" }
                    input type="text" value=(contact_name) placeholder="自动填充" readonly {}
                }
                div class="form-field" {
                    label { "联系电话" }
                    input type="text" value=(contact_phone) placeholder="自动填充" readonly {}
                }
            }
            div class="form-grid" style="margin-top:var(--space-3)" {
                div class="form-field" {
                    label { "收货地址" }
                    input type="text" name="shipping_address" value=(shipping_address) placeholder="选择客户后自动填充" {}
                }
            }
        }
    }
}

fn order_search_results(
    orders: &[abt_core::sales::sales_order::model::SalesOrder],
    items_map: &HashMap<i64, Vec<&OrderItemRow>>,
) -> Markup {
    html! {
        div class="product-select-list" {
            @for order in orders {
                @let status_text = order_status_text(order.status);
                @let order_date = order.order_date.format("%Y-%m-%d").to_string();
                @let total = order.total_amount.to_string();
                @let items_json = serde_json::json!({
                    "id": order.id,
                    "doc_number": &order.doc_number,
                    "items": items_map.get(&order.id).map(|items| items.iter().map(|item| {
                        serde_json::json!({
                            "order_item_id": item.order_item_id,
                            "product_id": item.product_id,
                            "product_code": &item.product_code,
                            "product_name": &item.product_name,
                            "specification": item.specification.as_deref().unwrap_or(""),
                            "unit": item.unit.as_deref().unwrap_or(""),
                            "ordered_qty": item.ordered_qty.to_string(),
                            "shipped_qty": item.shipped_qty.to_string(),
                        })
                    }).collect::<Vec<_>>()).unwrap_or_default()
                }).to_string();

                div class="product-select-item" {
                    div class="product-select-info" {
                        div class="product-select-name" { (order.doc_number) }
                        div class="product-select-meta" {
                            span { (order_date) }
                            span class="product-select-sep" { "·" }
                            span { (status_text) }
                            span class="product-select-sep" { "·" }
                            span { "¥" (total) }
                        }
                    }
                    button type="button" class="btn btn-sm btn-primary"
                        data-order=(items_json)
                        x-on:click="selectOrder(JSON.parse($el.dataset.order))" {
                        "选择"
                    }
                }
            }
        }
    }
}

fn order_search_empty() -> Markup {
    html! {
        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
            (icon::package_icon("w-8 h-8"))
            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "请先选择客户，或未找到匹配的订单" }
        }
    }
}
