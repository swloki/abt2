use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::purchase::return_order::model::*;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_return::*;
use crate::utils::RequestContext;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct OrderItemsParams {
    pub order_id: i64,
}

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct PRCreateForm {
    pub order_id: i64,
    pub return_date: String,
    pub return_reason: String,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
    order_item_id: i64,
    product_id: i64,
    returned_qty: String,
    unit_price: String,
}

// ── Handlers ──

#[require_permission("PURCHASE_RETURN", "create")]
pub async fn get_pr_create(
    _path: PRCreatePath,
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
    let order_svc = state.purchase_order_service();

    // Fetch Draft + Confirmed purchase orders
    let mut draft_orders = order_svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseOrderQuery {
                status: Some(PurchaseOrderStatus::Draft),
                ..Default::default()
            },
            PageParams::new(1, 100),
        )
        .await?;

    let confirmed_orders = order_svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseOrderQuery {
                status: Some(PurchaseOrderStatus::Confirmed),
                ..Default::default()
            },
            PageParams::new(1, 100),
        )
        .await?;

    draft_orders.items.extend(confirmed_orders.items);
    let orders = draft_orders.items;

    let content = pr_create_page(&orders);
    let page_html = admin_page(
        is_htmx,
        "新建采购退货",
        &claims,
        "purchase",
        PRCreatePath::PATH,
        "采购管理",
        Some("新建采购退货"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: fetch order items for a selected purchase order
#[require_permission("PURCHASE_RETURN", "create")]
pub async fn get_pr_order_items(
    ctx: RequestContext,
    Query(params): Query<OrderItemsParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let order_svc = state.purchase_order_service();
    let items = order_svc
        .list_items(&service_ctx, &mut conn, params.order_id)
        .await?;

    // Collect product IDs and batch-fetch product info
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_svc = state.product_service();
    let products = if product_ids.is_empty() {
        vec![]
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await?
    };
    let product_map: std::collections::HashMap<i64, &abt_core::master_data::product::model::Product> =
        products.iter().map(|p| (p.product_id, p)).collect();

    Ok(Html(order_items_fragment(&items, &product_map).into_string()))
}

/// POST: create purchase return from form submission (HTMX)
#[require_permission("PURCHASE_RETURN", "create")]
pub async fn create_pr(
    _path: PRCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PRCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    if form.order_id == 0 {
        return Err(DomainError::validation("请选择采购订单").into());
    }

    let order_svc = state.purchase_order_service();
    let order = order_svc
        .get(&service_ctx, &mut conn, form.order_id)
        .await?;

    let return_date = chrono::NaiveDate::parse_from_str(&form.return_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效退货日期格式: {e}")))?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效退货明细数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个退货产品").into());
    }

    let items: Vec<CreateReturnItemRequest> = web_items
        .into_iter()
        .map(|item| CreateReturnItemRequest {
            order_item_id: item.order_item_id,
            product_id: item.product_id,
            returned_qty: item
                .returned_qty
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            unit_price: item
                .unit_price
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
        })
        .collect();

    let create_req = CreatePurchaseReturnRequest {
        order_id: form.order_id,
        supplier_id: order.supplier_id,
        return_date,
        return_reason: form.return_reason,
        remark: form.remark.unwrap_or_default(),
        items,
    };

    let svc = state.purchase_return_service();
    let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

    let redirect = PRDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn pr_create_page(
    orders: &[abt_core::purchase::order::model::PurchaseOrder],
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    html! {
        div id="pr-app" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(PRListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回采购退货列表"
                }
                h1 class="page-title" { "新建采购退货" }
            }

            form id="pr-form"
                  hx-post=(PRCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="items_json";
                input type="hidden" name="order_id";

            // ── Order Selection ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "关联单据" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "采购订单" span style="color:var(--danger)" { "*" } }
                        select id="pr-order-select"
                            onchange="loadOrderItems()" {
                            option value="" { "请选择采购订单" }
                            @for o in orders {
                                @let status_text = order_status_text(o.status);
                                option value=(o.id) { (o.doc_number) " — " (status_text) }
                            }
                        }
                    }
                }
            }

            // ── Return Info ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "退货信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "退货日期" }
                        input type="date" name="return_date" value=(today) {}
                    }
                    div class="form-field" {
                        label { "退货原因" span style="color:var(--danger)" { "*" } }
                        select name="return_reason" required id="pr-return-reason" {
                            option value="" { "请选择" }
                            option value="质量问题" { "质量问题" }
                            option value="数量不符" { "数量不符" }
                            option value="规格错误" { "规格错误" }
                            option value="供应商取消" { "供应商取消" }
                            option value="其他" { "其他" }
                        }
                    }
                    // TODO: Rewrite conditional display with vanilla JS
                    div class="form-field" style="display:none" {
                        label { "具体原因" span style="color:var(--danger)" { "*" } }
                        input type="text" id="pr-return-reason-detail"
                            placeholder="请输入具体退货原因"
                            maxlength="200" {}
                    }
                }
            }
            // TODO: Rewrite computed return_reason hidden input with vanilla JS
            input type="hidden" name="return_reason";

            // ── Line Items ──
            // TODO: Rewrite conditional display with vanilla JS
            div class="data-card" style="display:none;padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="form-section-title" style="margin:0;padding:0;border:none" { "退货明细" }
                }
                div style="overflow-x:auto" {
                    table class="data-table" style="min-width:700px" {
                        thead {
                            tr {
                                th style="width:36px;text-align:center" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th class="num-right" { "订单数量" }
                                th class="num-right" { "单价" }
                                th style="width:120px;text-align:right" { "退货数量" }
                                th style="width:36px" { }
                            }
                        }
                        tbody {
                            // TODO: Rewrite x-for loop with vanilla JS rendering
                        }
                    }
                }
            }

            // ── Remark ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "备注" }
                textarea name="remark" placeholder="输入退货相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
            }

            // ── Action Bar ──
            div class="create-action-bar" {
                a class="btn btn-default" href=(PRListPath::PATH) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="submit" class="btn btn-primary" {
                        "提交退货"
                    }
                }
            }
            }

        }
    }
}

/// Order items fragment rendered when a PO is selected
fn order_items_fragment(
    items: &[abt_core::purchase::order::model::PurchaseOrderItem],
    product_map: &std::collections::HashMap<i64, &abt_core::master_data::product::model::Product>,
) -> Markup {
    html! {
        @for item in items {
            @let product = product_map.get(&item.product_id);
            @let product_code = product.map(|p| p.product_code.as_str()).unwrap_or("");
            @let product_name = product.map(|p| p.pdt_name.as_str()).unwrap_or(item.description.as_str());
            @let item_json = serde_json::json!({
                "order_item_id": item.id,
                "product_id": item.product_id,
                "product_code": product_code,
                "product_name": product_name,
                "order_qty": item.quantity.to_string(),
                "unit_price": item.unit_price.to_string(),
                "returned_qty": item.quantity.to_string(),
            }).to_string();

            div data-item=(item_json) {}
        }
    }
}

fn order_status_text(s: PurchaseOrderStatus) -> &'static str {
    match s {
        PurchaseOrderStatus::Draft => "草稿",
        PurchaseOrderStatus::Confirmed => "已确认",
        PurchaseOrderStatus::PartiallyReceived => "部分收货",
        PurchaseOrderStatus::Received => "已收货",
        PurchaseOrderStatus::Closed => "已关闭",
        PurchaseOrderStatus::Cancelled => "已取消",
    }
}
