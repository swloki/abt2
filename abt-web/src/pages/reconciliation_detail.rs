use std::collections::{HashMap, HashSet};

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::sales::reconciliation::model::*;
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::wms::picking::PickingService;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::routes::order::OrderDetailPath;
use crate::routes::shipping::ShippingDetailPath;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: ReconciliationStatus) -> (&'static str, &'static str) {
 match s {
 ReconciliationStatus::Draft => ("草稿", "status-draft"),
 ReconciliationStatus::Sent => ("已发送", "status-sent"),
 ReconciliationStatus::Confirmed => ("已确认", "status-confirmed"),
 ReconciliationStatus::Disputed => ("有异议", "status-disputed"),
 ReconciliationStatus::Settled => ("已结算", "status-settled"),
 }
}

struct ProductDetail {
 code: String,
 name: String,
 unit: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_reconciliation_detail(
 path: ReconciliationDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 let customer_svc = state.customer_service();
 let order_svc = state.sales_order_service();
 let shipping_svc = state.picking_service();
 let product_svc = state.product_service();
 let user_svc = state.user_service();

 let rec = reconciliation_svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

 let items = reconciliation_svc.list_items(&service_ctx, &mut conn, path.id).await?;

 let customer_name = customer_svc
 .get(&service_ctx, &mut conn, rec.customer_id)
 .await
 .map(|c| c.name)
 .unwrap_or_else(|_| "未知客户".into());

 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, rec.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // Resolve product details via service
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 let product_details: HashMap<i64, ProductDetail> = if product_ids.is_empty() {
 HashMap::new()
 } else {
 product_svc
 .get_by_ids(&service_ctx, &mut conn, product_ids)
 .await
 .map(|products| {
 products
 .into_iter()
 .map(|p| {
 (
 p.product_id,
 ProductDetail {
 code: p.product_code,
 name: p.pdt_name,
 unit: Some(p.unit),
 },
 )
 })
 .collect()
 })
 .unwrap_or_default()
 };

 // Resolve order numbers via service (deduplicated)
 let order_numbers: HashMap<i64, String> = {
 let mut map = HashMap::new();
 let mut seen = HashSet::new();
 for item in &items {
 if seen.insert(item.sales_order_id)
 && let Ok(order) = order_svc.find_by_id(&service_ctx, &mut conn, item.sales_order_id).await {
 map.insert(item.sales_order_id, order.doc_number);
 }
 }
 map
 };

 // Resolve shipping numbers via service (deduplicated)
 let shipping_numbers: HashMap<i64, String> = {
 let mut map = HashMap::new();
 let mut seen = HashSet::new();
 for item in &items {
 if seen.insert(item.shipping_request_id)
 && let Ok(shipping) = shipping_svc.find_by_id(&service_ctx, &mut conn, item.shipping_request_id).await {
 map.insert(item.shipping_request_id, shipping.doc_number);
 }
 }
 map
 };

 let content = reconciliation_detail_page(&rec, &items, &customer_name, &operator_name, &product_details, &order_numbers, &shipping_numbers);
 let page_html = admin_page(
 is_htmx, "对账详情", &claims, "sales",
 &format!("{}/{}", ReconciliationListPath::PATH, path.id),
 "销售管理", Some("对账详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

pub async fn send_reconciliation(
 path: SendReconciliationPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let reconciliation_svc = state.reconciliation_service();
 reconciliation_svc.send(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ReconciliationDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_reconciliation(
 path: ConfirmReconciliationPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 reconciliation_svc.confirm(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ReconciliationDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn dispute_reconciliation(
 path: DisputeReconciliationPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 reconciliation_svc.dispute(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ReconciliationDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn settle_reconciliation(
 path: SettleReconciliationPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 reconciliation_svc.settle(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ReconciliationDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: ReconciliationStatus) -> Markup {
 let steps: &[(&str, ReconciliationStatus)] = &[
 ("草稿", ReconciliationStatus::Draft),
 ("已发送", ReconciliationStatus::Sent),
 ("已确认", ReconciliationStatus::Confirmed),
 ("已结算", ReconciliationStatus::Settled),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_disputed = current == ReconciliationStatus::Disputed;

 html! {
    div class="flex items-center mt-6 mb-6" {
        @for (i, (label, _)) in steps.iter().enumerate() {
            @if i > 0 {
                div class=({
                        format!(
                            "w-[48px] h-[2px] {}",
                            if i <= current_idx && !is_disputed {
                                "bg-success"
                            } else {
                                "bg-border"
                            },
                        )
                    }) {}
            }
            @let (dot_cls, text_cls, ring_cls) = if is_disputed {
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
        @if is_disputed {
            div class="w-[48px] h-[2px] bg-border" {}
            div class="flex items-center gap-2 shrink-0" {
                span class="w-2.5 h-2.5 rounded-full shrink-0 bg-danger-500" {}
                span class="text-xs text-danger-500 font-semibold whitespace-nowrap" { "有异议" }
            }
        }
    }
}
}

// ── Components ──

fn reconciliation_detail_page(
 rec: &Reconciliation,
 items: &[ReconciliationItem],
 customer_name: &str,
 operator_name: &str,
 product_details: &HashMap<i64, ProductDetail>,
 order_numbers: &HashMap<i64, String>,
 shipping_numbers: &HashMap<i64, String>,
) -> Markup {
 let (status_text, status_class) = status_label(rec.status);

 html! {
    div {
        // ── Back Link ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{}?restore=true", ReconciliationListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回对账单列表" }
        // ── Detail Header ──
        div class="flex items-center justify-between bg-bg border border-border-soft rounded-lg p-6"
        {
            div {
                div class="flex items-center justify-between" {
                    h1 class="text-2xl font-extrabold font-mono tabular-nums" { (rec.doc_number) }
                    span class=({
                        format!(
                            "status-pill {}",
                            crate::utils::status_color(status_class),
                        )
                    }) { (status_text) }
                }
                div class="text-[13px] text-muted mt-2" {
                    "对账期间："
                    (rec.period.as_str())
                    "　客户："
                    (customer_name)
                }
            }
            div class="flex gap-3" {
                @if rec.status == ReconciliationStatus::Draft {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-post=({
                            SendReconciliationPath {
                                id: rec.id,
                            }
                                .to_string()
                        })
                        hx-confirm="确认发送此对账单？"
                    { "发送对账" }
                }
                @if rec.status == ReconciliationStatus::Sent {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-success text-white"
                        hx-post=({
                            ConfirmReconciliationPath {
                                id: rec.id,
                            }
                                .to_string()
                        })
                        hx-confirm="确认此对账单？"
                    { "确认" }
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                        hx-post=({
                            DisputeReconciliationPath {
                                id: rec.id,
                            }
                                .to_string()
                        })
                        hx-confirm="确认提出异议？"
                    { "异议" }
                }
                @if rec.status == ReconciliationStatus::Confirmed {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-success text-white"
                        hx-post=({
                            SettleReconciliationPath {
                                id: rec.id,
                            }
                                .to_string()
                        })
                        hx-confirm="确认结算？"
                    { "结算" }
                }
            }
        }
        // ── Workflow Steps ──
        (workflow_steps(rec.status))
        // ── Summary Cards ──
        div class="grid grid-cols-3 gap-4 mb-6" {
            div class="bg-bg border border-border-soft rounded-lg p-4" {
                div class="text-xs text-muted font-medium" { "总金额" }
                div class="font-mono tabular-nums text-lg font-bold text-fg mt-1" {
                    (crate::utils::fmt_amount(rec.total_amount))
                }
            }
            div class="bg-bg border border-border-soft rounded-lg p-4" {
                div class="text-xs text-muted font-medium" { "确认金额" }
                div class="font-mono tabular-nums text-lg font-bold text-success mt-1" {
                    (crate::utils::fmt_amount(rec.confirmed_amount))
                }
            }
            div class="bg-bg border border-border-soft rounded-lg p-4" {
                div class="text-xs text-muted font-medium" { "差额" }
                div class="font-mono tabular-nums text-lg font-bold text-danger mt-1" {
                    (crate::utils::fmt_amount(rec.difference))
                }
            }
        }
        // ── Info ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "对账信息"
            }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "客户名称" }
                    span class="text-sm text-fg font-medium" { (customer_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "对账期间" }
                    span class="text-sm text-fg font-medium" { (rec.period.as_str()) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "操作员" }
                    span class="text-sm text-fg font-medium" { (operator_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "创建时间" }
                    span class="text-sm text-fg font-medium" {
                        (rec.created_at.format("%Y-%m-%d %H:%M"))
                    }
                }
            }
        }
        // ── Items Table ──
        div class="data-card" {
            table class="data-table" {
                thead {
                    tr {
                        th { "来源类型" }
                        th { "来源单号" }
                        th { "关联订单" }
                        th { "产品编码" }
                        th { "产品名称" }
                        th { "单位" }
                        th class="text-right text-[13px]" { "数量" }
                        th class="text-right text-[13px]" { "单价" }
                        th class="text-right text-[13px]" { "金额" }
                        th { "确认" }
                    }
                }
                tbody {
                    @for item in items {
                        ({
                            item_row(
                                item,
                                product_details,
                                order_numbers,
                                shipping_numbers,
                            )
                        })
                    }
                    @if items.is_empty() {
                        tr {
                            td colspan="10" class="text-center p-8 text-muted" { "暂无明细" }
                        }
                    }
                }
            }
        }
        // ── Amount Summary ──
        div class="flex justify-end gap-8 p-5 border-t border-border-soft bg-surface-raised" {
            div class="flex gap-3" {
                span class="text-[11px] text-muted font-medium uppercase" { "确认金额" }
                span class="text-[20px] font-bold text-fg text-success" {
                    (crate::utils::fmt_amount(rec.confirmed_amount))
                }
            }
            div class="flex gap-3" {
                span class="text-[11px] text-muted font-medium uppercase" { "差异金额" }
                span class="text-[20px] font-bold text-fg" {
                    (crate::utils::fmt_amount(rec.difference))
                }
            }
            div class="flex gap-3" {
                span class="text-[11px] text-muted font-medium uppercase" { "对账净额" }
                span class="text-[20px] font-bold text-accent" {
                    (crate::utils::fmt_amount(rec.total_amount))
                }
            }
        }
        // ── Remarks ──
        @if !rec.remark.is_empty() {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] mt-6"
            {
                div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                    "备注"
                }
                p class="text-muted" { (rec.remark.as_str()) }
            }
        }
    }
}
}

fn item_row(
 item: &ReconciliationItem,
 product_details: &HashMap<i64, ProductDetail>,
 order_numbers: &HashMap<i64, String>,
 shipping_numbers: &HashMap<i64, String>,
) -> Markup {
 let detail = product_details.get(&item.product_id);
 let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
 let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
 let unit = detail.and_then(|d| d.unit.as_deref()).unwrap_or("—");
 let order_num = order_numbers.get(&item.sales_order_id).map(|s| s.as_str()).unwrap_or("—");
 let shipping_num = shipping_numbers.get(&item.shipping_request_id).map(|s| s.as_str()).unwrap_or("—");
 let shipping_detail = ShippingDetailPath { id: item.shipping_request_id };
 let order_detail = OrderDetailPath { id: item.sales_order_id };

 html! {
    tr {
        td { "发货" }
        td {
            a href=(shipping_detail.to_string()) class="text-info" { (shipping_num) }
        }
        td {
            a href=(order_detail.to_string()) class="text-info" { (order_num) }
        }
        td class="font-mono tabular-nums" { (product_code) }
        td { (product_name) }
        td { (unit) }
        td class="text-right text-[13px]" { (fmt_qty(item.quantity)) }
        td class="text-right text-[13px] font-mono tabular-nums" {
            (format!("{:.2}", item.unit_price))
        }
        td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.amount)) }
        td {
            @if item.confirmed {
                span class="text-success" { "已确认" }
            } @else {
                span class="text-muted" { "未确认" }
            }
        }
    }
}
}
