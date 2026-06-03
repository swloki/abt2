use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::{SalesOrderQuery, SalesOrderStatus};
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::sales_return::model::{
    CreateReturnItemReq, CreateReturnReq, ReturnDisposition,
};
use abt_core::sales::sales_return::SalesReturnService;
use abt_core::sales::shipping_request::model::ShippingQuery;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::sales_return::*;
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

// ── Form & Query Structs ──

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReturnCreateForm {
    pub order_id: i64,
    pub shipping_request_id: i64,
    pub customer_id: i64,
    pub return_reason: String,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReturnItemWeb {
    order_item_id: i64,
    product_id: i64,
    returned_qty: String,
    disposition: i16,
}

#[derive(Debug, Deserialize)]
pub struct OrderSearchQuery {
    pub customer_id: Option<i64>,
    pub keyword: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "create")]
pub async fn get_return_create(
    _path: ReturnCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

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

    let content = return_create_page(&customers.items);
    let page_html = admin_page(
        is_htmx,
        "新建退货单",
        &claims,
        "sales",
        ReturnCreatePath::PATH,
        "销售管理",
        Some("新建退货单"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search orders -> returns HTML fragment with embedded JSON data
#[require_permission("SALES_ORDER", "read")]
pub async fn get_orders(
    ctx: RequestContext,
    Query(params): Query<OrderSearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let customer_id = match params.customer_id {
        Some(id) if id > 0 => id,
        _ => return Ok(Html(order_search_empty().into_string())),
    };

    // 1. Fetch orders via SalesOrderService::list
    let order_svc = state.sales_order_service();
    let keyword = params.keyword.as_deref().and_then(|k| {
        if k.is_empty() {
            None
        } else {
            Some(k.to_string())
        }
    });
    let orders_result = order_svc
        .list(
            &service_ctx,
            &mut conn,
            SalesOrderQuery {
                customer_id: Some(customer_id),
                keyword,
                ..Default::default()
            },
            PageParams::new(1, 10),
        )
        .await?;

    // Filter to only active statuses (2=Confirmed, 3=InProduction, 4=PartiallyShipped, 5=Shipped)
    let active_statuses = [
        SalesOrderStatus::Confirmed,
        SalesOrderStatus::InProduction,
        SalesOrderStatus::PartiallyShipped,
        SalesOrderStatus::Shipped,
    ];
    let orders: Vec<_> = orders_result
        .items
        .into_iter()
        .filter(|o| active_statuses.contains(&o.status))
        .collect();

    if orders.is_empty() {
        return Ok(Html(order_search_empty().into_string()));
    }

    let order_ids: Vec<i64> = orders.iter().map(|o| o.id).collect();

    // 2. Fetch order items for each order via SalesOrderService::list_items
    let mut items_map: std::collections::HashMap<i64, Vec<abt_core::sales::sales_order::model::SalesOrderItem>> =
        std::collections::HashMap::new();
    for &oid in &order_ids {
        let items = order_svc
            .list_items(&service_ctx, &mut conn, oid)
            .await?;
        items_map.insert(oid, items);
    }

    // 3. Collect all unique product IDs and batch-fetch product info
    let all_product_ids: Vec<i64> = items_map
        .values()
        .flat_map(|items| items.iter().map(|i| i.product_id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let product_svc = state.product_service();
    let products = if all_product_ids.is_empty() {
        vec![]
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, all_product_ids)
            .await?
    };
    let product_map: std::collections::HashMap<i64, &abt_core::master_data::product::model::Product> =
        products.iter().map(|p| (p.product_id, p)).collect();

    // 4. Resolve shipping IDs for these orders (latest per order)
    let shipping_svc = state.shipping_service();
    let mut shipping_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &oid in &order_ids {
        let shippings = shipping_svc
            .list(
                &service_ctx,
                &mut conn,
                ShippingQuery {
                    order_id: Some(oid),
                    ..Default::default()
                },
                PageParams::new(1, 100),
            )
            .await?;
        // Take the latest shipping (highest ID) as the original DISTINCT ON logic did
        if let Some(latest) = shippings.items.iter().max_by_key(|s| s.id) {
            shipping_map.insert(oid, latest.id);
        }
    }

    Ok(Html(
        order_search_results(&orders, &items_map, &product_map, &shipping_map).into_string(),
    ))
}

/// POST: create return from form submission
#[require_permission("SALES_ORDER", "create")]
pub async fn create_return(
    _path: ReturnCreatePath,
    ctx: RequestContext,
    Form(form): Form<ReturnCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { claims: _, mut conn, state, service_ctx, .. } = ctx;

    if form.customer_id == 0 {
        return Err(DomainError::validation("请选择客户").into());
    }
    if form.order_id == 0 {
        return Err(DomainError::validation("请选择来源订单").into());
    }

    let web_items: Vec<ReturnItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效退货明细数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个退货产品").into());
    }

    // Build CreateReturnReq for the service
    let items: Vec<CreateReturnItemReq> = web_items
        .into_iter()
        .map(|item| {
            let qty: rust_decimal::Decimal = item
                .returned_qty
                .parse()
                .unwrap_or(rust_decimal::Decimal::ONE);
            let disposition = ReturnDisposition::from_i16(item.disposition)
                .unwrap_or(ReturnDisposition::Restock);
            CreateReturnItemReq {
                order_item_id: item.order_item_id,
                returned_qty: qty,
                disposition,
            }
        })
        .collect();

    let req = CreateReturnReq {
        order_id: form.order_id,
        shipping_request_id: form.shipping_request_id,
        customer_id: form.customer_id,
        return_reason: form.return_reason,
        items,
    };

    let svc = state.sales_return_service();
    let return_id = svc.create(&service_ctx, &mut conn, req).await?;

    let redirect = ReturnDetailPath { id: return_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn return_create_page(customers: &[abt_core::master_data::customer::model::Customer]) -> Markup {
    html! {
        div id="return-app" {
            div class="page-header" {
                a class="back-link" href=(ReturnListPath::PATH) {
                    (icon::chevron_left_icon("w-4 h-4"))
                    "返回退货列表"
                }
                h1 class="page-title" { "新建退货单" }
            }

            form id="return-form"
                  hx-post=(ReturnCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="items_json";
                input type="hidden" name="customer_id";
                input type="hidden" name="order_id";
                input type="hidden" name="shipping_request_id";

                // ── Customer ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "客户信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "客户名称" span style="color:var(--danger)" { "*" } }
                            select id="return-customer-select" {
                                option value="0" { "请选择客户" }
                                @for c in customers {
                                    option value=(c.id) { (c.name) }
                                }
                            }
                        }
                    }
                }

                // ── Order Picker ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "关联单据" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "选择订单" span style="color:var(--danger)" { "*" } }
                            div style="display:flex;gap:var(--space-2)" {
                                input type="text" readonly
                                input type="text" readonly
                                    id="return-order-number"
                                    placeholder="选择客户后可选择订单"
                                    style="flex:1;cursor:pointer" {}
                                button type="button" class="btn btn-sm btn-default"
                                // TODO: Rewrite conditional display with vanilla JS
                                button type="button" class="btn btn-sm btn-default" style="display:none"
                                    id="return-clear-order-btn" title="清除" {
                                    (icon::x_icon("w-3.5 h-3.5"))
                                }
                                button type="button" class="btn btn-sm btn-primary"
                                button type="button" class="btn btn-sm btn-primary"
                                    _="on click add .is-open to #order-modal" {
                                    "选择订单"
                                }
                            }
                        }
                        div class="form-field" {
                            label { "退货原因" span style="color:var(--danger)" { "*" } }
                            select id="return-reason-select" {
                                option value="" { "请选择" }
                                option value="质量问题" { "质量问题" }
                                option value="数量不符" { "数量不符" }
                                option value="规格错误" { "规格错误" }
                                option value="客户取消" { "客户取消" }
                                option value="其他" { "其他" }
                            }
                        }
                        // TODO: Rewrite conditional display with vanilla JS
                        div class="form-field" style="display:none" {
                            label { "具体原因" span style="color:var(--danger)" { "*" } }
                            input type="text" id="return-reason-detail"
                                placeholder="请输入具体退货原因"
                                maxlength="200" {}
                        }
                    }
                }
                // TODO: Rewrite computed return_reason hidden input with vanilla JS
                input type="hidden" name="return_reason";

                // ── Return Items ──
                // TODO: Rewrite conditional display with vanilla JS
                div class="data-card" style="display:none;padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                    div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                        span class="form-section-title" style="margin:0;padding:0;border:none" { "退货明细" }
                    }
                    div style="overflow-x:auto" {
                        table class="data-table" style="min-width:700px" {
                            thead {
                                tr {
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th { "单位" }
                                    th class="num-right" { "订单数量" }
                                    th class="num-right" { "单价" }
                                    th style="width:100px;text-align:right" { "退货数量" }
                                    th style="width:120px" { "处理方式" }
                                    th style="width:36px" { }
                                }
                            }
                            tbody {
                        tbody {
                            // TODO: Rewrite x-for loop with vanilla JS rendering
                        }
                            }
                        }
                    }
                }

                // ── Remark ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "备注" }
                    textarea name="remark" placeholder="输入退货备注…"
                        style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(ReturnListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-primary" {
                        "提交退货"
                    }
                }
            }

            // ── Order Picker Modal ──
            div class="modal-overlay" id="order-modal"
                _="on click remove .is-open from #order-modal" {
                div class="modal modal-lg" onclick="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "选择来源订单" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            _="on click remove .is-open from #order-modal" { "x" }
                    }
                    div class="modal-body" style="padding:0" {
                        div class="product-search-bar" {
                            input type="hidden" name="customer_id" {}
                            div class="product-search-field" {
                                label class="product-search-label" { "搜索订单" }
                                input class="product-search-input" type="text" name="keyword" placeholder="输入订单号…"
                                    hx-get=(ReturnOrdersPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#return-order-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar input" {}
                            }
                        }
                        div id="return-order-results" style="max-height:360px;overflow-y:auto"
                            hx-get=(ReturnOrdersPath::PATH)
                            hx-trigger="intersect once"
                            hx-include=".product-search-bar input"
                            hx-swap="innerHTML" {
                            div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                                "加载中..."
                            }
                        }
                    }
                }
            }

        }
    }
}

fn order_search_results(
    orders: &[abt_core::sales::sales_order::model::SalesOrder],
    items_map: &std::collections::HashMap<i64, Vec<abt_core::sales::sales_order::model::SalesOrderItem>>,
    product_map: &std::collections::HashMap<i64, &abt_core::master_data::product::model::Product>,
    shipping_map: &std::collections::HashMap<i64, i64>,
) -> Markup {
    html! {
        div class="product-select-list" {
            @for order in orders {
                @let status_text = order_status_text(order.status);
                @let order_date = order.order_date.format("%Y-%m-%d").to_string();
                @let total = order.total_amount.to_string();
                @let shipping_id = shipping_map.get(&order.id).copied().unwrap_or(0);
                @let items_json = serde_json::json!({
                    "id": order.id,
                    "doc_number": &order.doc_number,
                    "shipping_id": shipping_id,
                    "items": items_map.get(&order.id).map(|items| items.iter().map(|item| {
                        let product = product_map.get(&item.product_id);
                        serde_json::json!({
                            "order_item_id": item.id,
                            "product_id": item.product_id,
                            "product_code": product.map(|p| p.product_code.as_str()).unwrap_or(""),
                            "product_name": product.map(|p| p.pdt_name.as_str()).unwrap_or_else(|| item.description.as_str()),
                            "unit": product.map(|p| p.unit.as_str()).unwrap_or(""),
                            "order_qty": item.quantity.to_string(),
                            "unit_price": item.unit_price.to_string(),
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
                        onclick="selectOrder(JSON.parse(this.dataset.order))" {
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
