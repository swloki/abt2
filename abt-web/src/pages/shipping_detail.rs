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
 let RequestContext { state, service_ctx, .. } = ctx;

 // confirm 是多步写（状态机 + status + 审计），事务包裹防半失败残留
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.shipping_service();
 svc.confirm(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ShippingDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn pick_shipping(
 path: PickShippingPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 // pick 是多步写（状态机 + status + 审计），事务包裹防半失败残留
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.shipping_service();
 svc.pick(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ShippingDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn ship_shipping(
 path: ShipShippingPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 // ship 是多步写：改 shipped_qty → 释放预留 → 出库(SalesShipment) → COGS → AR立账 → 状态机。
 // 必须事务包裹：半失败时整体回滚，避免「数量已提交但库存/AR/状态未动」的残留
 // （事故案例 SO-2026-06-000170 / SR-2026-06-000043：product_id=0 触发出库预检报错，
 // 已提交的 shipped_qty 无法回滚，发货单卡在 Picking 却显示已发数量）
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.shipping_service();
 svc.ship(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ShippingDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn cancel_shipping(
 path: CancelShippingPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 // cancel 是多步写（状态机 + status + 审计），事务包裹防半失败残留
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.shipping_service();
 svc.cancel(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

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
    div class="flex items-center mt-6 mb-6" {
        @for (i, (label, _)) in steps.iter().enumerate() {
            @if i > 0 {
                div class=({
                        format!(
                            "w-[48px] h-[2px] {}",
                            if i <= current_idx && !is_cancelled {
                                "bg-success"
                            } else {
                                "bg-border"
                            },
                        )
                    }) {}
            }
            @let (dot_cls, text_cls, ring_cls) = if is_cancelled {
                ("bg-border-soft", "text-muted", "")
            } else if i < current_idx {
                ("bg-success", "text-success", "")
            } else if i == current_idx {
                (
                    "bg-accent",
                    "text-accent font-semibold",
                    "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]",
                )
            } else {
                ("bg-slate-300", "text-slate-400", "")
            };
            div class="flex items-center gap-2 shrink-0" {
                span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
                span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
            }
        }
        @if is_cancelled {
            div class="w-[48px] h-[2px] bg-border" {}
            div class="flex items-center gap-2 shrink-0" {
                span class="w-2.5 h-2.5 rounded-full shrink-0 bg-danger-500" {}
                span class="text-xs text-danger-500 font-semibold whitespace-nowrap" { "已取消" }
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
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{}?restore=true", ShippingListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回发货申请列表" }
        // ── Detail Header ──
        div class="block bg-bg border border-border-soft rounded-lg p-6" {
            div {
                div class="flex items-center justify-between" {
                    h1 class="text-2xl font-extrabold font-mono tabular-nums" { (s.doc_number) }
                    span class=({
                        format!(
                            "status-pill {}",
                            crate::utils::status_color(status_class),
                        )
                    }) { (status_text) }
                }
                div class="text-[13px] text-muted" {
                    "来源订单："
                    @if let Some(oid) = s.order_id {
                        a href=(format!("/admin/orders/{oid}")) { (order_number) }
                    } @else { (order_number) }
                }
            }
            div class="flex gap-3" {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", ShippingListPath::PATH))
                { "返回列表" }
                @if s.status == ShippingStatus::Draft {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-post=(ConfirmShippingPath { id: s.id }.to_string())
                        hx-confirm="确认审核此发货单？"
                    { "确认发货" }
                }
                @if s.status == ShippingStatus::Confirmed {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-post=(PickShippingPath { id: s.id }.to_string())
                        hx-confirm="确认开始拣货？"
                    { "开始拣货" }
                }
                @if s.status == ShippingStatus::Picking {
                    button
                        class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-success text-white"
                        hx-post=(ShipShippingPath { id: s.id }.to_string())
                        hx-confirm="确认已发出？"
                    { "确认发出" }
                }
                @if matches!(s.status, ShippingStatus::Draft | ShippingStatus::Confirmed) {
                    button
                        class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                        hx-post=(CancelShippingPath { id: s.id }.to_string())
                        hx-confirm="确认取消此发货单？"
                    { "取消" }
                }
            }
        }
        // ── Workflow Steps ──
        (workflow_steps(s.status))
        // ── Shipping Info ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "发货信息"
            }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "客户名称" }
                    span class="text-sm text-fg font-medium" { (customer_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "收货地址" }
                    span class="text-sm text-fg font-medium" { (s.shipping_address.as_str()) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "预计发货日期" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        ({
                            s.expected_ship_date
                                .map(|d| d.format("%Y-%m-%d").to_string())
                                .unwrap_or_else(|| "—".into())
                        })
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "承运商" }
                    span class="text-sm text-fg font-medium" { (s.carrier.as_str()) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "物流单号" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        (s.tracking_number.as_str())
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "操作员" }
                    span class="text-sm text-fg font-medium" { (operator_name) }
                }
            }
        }
        // ── Items Table ──
        div class="data-card" {
            div class="overflow-x-auto" {
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
                        @for item in items { (item_row(item, product_details, warehouse_names)) }
                        @if items.is_empty() {
                            tr {
                                td colspan="8" class="text-center p-8 text-muted" { "暂无明细" }
                            }
                        }
                    }
                }
            }
        }
        // ── Remarks ──
        @if !s.remark.is_empty() {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] mt-6"
            {
                div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                    "备注"
                }
                p class="text-muted" { (s.remark.as_str()) }
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
