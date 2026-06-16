use std::collections::{HashMap, HashSet};

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::model::*;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::identity::UserService;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::shipping::*;
use crate::utils::fmt_qty;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: ShippingStatus) -> (&'static str, &'static str) {
    match s {
        ShippingStatus::Draft => ("草稿", "status-draft"),
        ShippingStatus::Confirmed => ("已确认", "status-confirmed"),
        ShippingStatus::Picking => ("拣货中", "status-picking"),
        ShippingStatus::Shipped => ("已发货", "status-shipped"),
        ShippingStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

struct ProductDetail {
    code: String,
    name: String,
    spec: Option<String>,
    unit: Option<String>,
}

// ── Handlers ──

#[require_permission("SHIPPING", "read")]
pub async fn get_shipping_detail(
    path: ShippingDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let shipping_svc = state.shipping_service();
    let customer_svc = state.customer_service();
    let order_svc = state.sales_order_service();
    let product_svc = state.product_service();
    let warehouse_svc = state.warehouse_service();
    let user_svc = state.user_service();
    let shipping = shipping_svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let items = shipping_svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let customer_name = customer_svc.get(&service_ctx, &mut conn, shipping.customer_id)
        .await.map(|c| c.name).unwrap_or_else(|_| "未知客户".into());
    let order_number = match shipping.order_id {
        Some(oid) => order_svc.find_by_id(&service_ctx, &mut conn, oid)
            .await.map(|o| o.doc_number).unwrap_or_else(|_| "—".into()),
        None => "—".into(),
    };
    let operator_name = user_svc.get_user(&service_ctx, &mut conn, shipping.operator_id)
        .await.map(|u| u.display_name.unwrap_or(u.username)).unwrap_or_else(|_| "—".into());

    // Resolve product details via product service
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_details: HashMap<i64, ProductDetail> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        product_svc.get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .map(|products| products.into_iter().map(|p| {
                (p.product_id, ProductDetail {
                    code: p.product_code,
                    name: p.pdt_name,
                    spec: Some(p.meta.specification),
                    unit: Some(p.unit),
                })
            }).collect())
            .unwrap_or_default()
    };

    // Resolve warehouse names via warehouse service
    let mut warehouse_names = HashMap::new();
    let mut seen_wh = HashSet::new();
    for item in &items {
        if seen_wh.insert(item.warehouse_id)
            && let Ok(wh) = warehouse_svc.get(&service_ctx, &mut conn, item.warehouse_id).await {
                warehouse_names.insert(item.warehouse_id, wh.name);
            }
    }

    let content = shipping_detail_page(&shipping, &items, &customer_name, &order_number, &operator_name, &product_details, &warehouse_names);
    let page_html = admin_page(
        is_htmx, "发货详情", &claims, "sales",
        &format!("{}/{}", ShippingListPath::PATH, path.id),
        "销售管理", Some("发货详情"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SHIPPING", "update")]
pub async fn confirm_shipping(
    path: ConfirmShippingPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let svc = state.shipping_service();
    svc.confirm(&service_ctx, &mut conn, path.id).await?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn pick_shipping(
    path: PickShippingPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let svc = state.shipping_service();
    svc.pick(&service_ctx, &mut conn, path.id).await?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn ship_shipping(
    path: ShipShippingPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let svc = state.shipping_service();
    svc.ship(&service_ctx, &mut conn, path.id).await?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn cancel_shipping(
    path: CancelShippingPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let svc = state.shipping_service();
    svc.cancel(&service_ctx, &mut conn, path.id).await?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: ShippingStatus) -> Markup {
    let steps: &[(&str, ShippingStatus)] = &[
        ("草稿", ShippingStatus::Draft),
        ("已确认", ShippingStatus::Confirmed),
        ("拣货中", ShippingStatus::Picking),
        ("已发货", ShippingStatus::Shipped),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == ShippingStatus::Cancelled;

    html! {
        div class="flex items-center" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx && !is_cancelled { "wf-line current" } else { "wf-line" }) {}
                }
                @let step_class = if is_cancelled {
                    "wf-step"
                } else if i < current_idx {
                    "wf-step completed"
                } else if i == current_idx {
                    "wf-step current"
                } else {
                    "wf-step"
                };
                div class=(step_class) {
                    div class="w-[10px] h-[10px] rounded-full bg-border" {}
                    (label)
                }
            }
            @if is_cancelled {
                div class="flex items-center gap-2 text-xs text-text-muted cancelled danger" {
                    div class="w-[10px] h-[10px] rounded-full bg-border" {}
                    "已取消"
                }
            }
        }
    }
}

// ── Components ──

fn shipping_detail_page(
    s: &ShippingRequest,
    items: &[ShippingRequestItem],
    customer_name: &str,
    order_number: &str,
    operator_name: &str,
    product_details: &HashMap<i64, ProductDetail>,
    warehouse_names: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(s.status);

    html! {
        div {
            // ── Back Link ──
            a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ShippingListPath::PATH)) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回发货申请列表"
            }

            // ── Detail Header ──
            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div {
                    div class="flex items-center justify-between" {
                        h1 class="text-2xl font-extrabold font-font-mono tabular-nums" { (s.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                    div class="text-[13px] text-text-muted" {
                        "来源订单："
                        @if let Some(oid) = s.order_id {
                            a href=(format!("/admin/orders/{oid}")) { (order_number) }
                        } @else {
                            (order_number)
                        }
                    }
                }
                div class="flex gap-3" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" href=(format!("{}?restore=true", ShippingListPath::PATH)) { "返回列表" }
                    @if s.status == ShippingStatus::Draft {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-post=(ConfirmShippingPath { id: s.id }.to_string())
                            hx-confirm="确认审核此发货单？" { "确认发货" }
                    }
                    @if s.status == ShippingStatus::Confirmed {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-post=(PickShippingPath { id: s.id }.to_string())
                            hx-confirm="确认开始拣货？" { "开始拣货" }
                    }
                    @if s.status == ShippingStatus::Picking {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-[#10b981] text-[#fff]"
                            hx-post=(ShipShippingPath { id: s.id }.to_string())
                            hx-confirm="确认已发出？" { "确认发出" }
                    }
                    @if matches!(s.status, ShippingStatus::Draft | ShippingStatus::Confirmed) {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                            hx-post=(CancelShippingPath { id: s.id }.to_string())
                            hx-confirm="确认取消此发货单？" { "取消" }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(s.status))

            // ── Shipping Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "发货信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "客户名称" }
                        span class="text-sm text-fg font-medium" { (customer_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "收货地址" }
                        span class="text-sm text-fg font-medium" { (s.shipping_address.as_str()) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "预计发货日期" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" {
                            (s.expected_ship_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                        }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "承运商" }
                        span class="text-sm text-fg font-medium" { (s.carrier.as_str()) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "物流单号" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (s.tracking_number.as_str()) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "操作员" }
                        span class="text-sm text-fg font-medium" { (operator_name) }
                    }
                }
            }

            // ── Items Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th { "单位" }
                                th class="text-right text-[13px]" { "申请数量" }
                                th class="text-right text-[13px]" { "已发货" }
                                th { "发货仓库" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_details, warehouse_names))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="8" class="text-center p-8 text-text-muted" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Remarks ──
            @if !s.remark.is_empty() {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] mt-6" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "备注" }
                    p class="text-text-muted" { (s.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &ShippingRequestItem,
    details: &HashMap<i64, ProductDetail>,
    warehouses: &HashMap<i64, String>,
) -> Markup {
    let detail = details.get(&item.product_id);
    let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
    let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
    let spec = detail.and_then(|d| d.spec.as_deref()).unwrap_or("—");
    let unit = detail.and_then(|d| d.unit.as_deref()).unwrap_or("—");
    let warehouse = warehouses.get(&item.warehouse_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td class="font-mono tabular-nums" { (item.line_no) }
            td class="font-mono tabular-nums" { (product_code) }
            td { (product_name) }
            td { (spec) }
            td { (unit) }
            td class="text-right text-[13px]" { (fmt_qty(item.requested_qty)) }
            td class="text-right text-[13px]" { (fmt_qty(item.shipped_qty)) }
            td { (warehouse) }
        }
    }
}
