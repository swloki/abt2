use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::model::AcquireChannel;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::{DemandService, SalesOrderService};
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::identity::UserService;
use abt_core::wms::stock_ledger::StockLedgerService;

const DECIMAL_100: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: SalesOrderStatus) -> (&'static str, &'static str) {
 match s {
 SalesOrderStatus::Draft => ("草稿", "status-draft"),
 SalesOrderStatus::Confirmed => ("已确认", "status-confirmed"),
 SalesOrderStatus::PartiallyShipped => ("部分发货", "status-progress"),
 SalesOrderStatus::Shipped => ("已发货", "status-shipped"),
 SalesOrderStatus::Completed => ("已完成", "status-completed"),
 SalesOrderStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

fn line_status_pill(s: SalesOrderLineStatus) -> (&'static str, &'static str) {
 match s {
 SalesOrderLineStatus::Pending => ("待处理", "status-pending"),
 SalesOrderLineStatus::Allocated => ("已分配", "status-confirmed"),
 SalesOrderLineStatus::Producing => ("生产中", "status-warn"),
 SalesOrderLineStatus::Purchasing => ("采购中", "status-purple"),
 SalesOrderLineStatus::Shipped => ("已发货", "status-success"),
 SalesOrderLineStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

fn fulfill_status_pill(s: FulfillmentLineStatus) -> (&'static str, &'static str) {
 match s {
 FulfillmentLineStatus::Pending => ("待处理", "status-pending"),
 FulfillmentLineStatus::Allocated => ("已分配", "status-confirmed"),
 FulfillmentLineStatus::Producing => ("生产中", "status-warn"),
 FulfillmentLineStatus::Purchasing => ("采购中", "status-purple"),
 FulfillmentLineStatus::Fulfilled => ("已履约", "status-success"),
 }
}

fn acquire_tag(ch: AcquireChannel) -> (&'static str, &'static str) {
 match ch {
 AcquireChannel::SelfProduced | AcquireChannel::Legacy => ("自制", "status-confirmed"),
 AcquireChannel::Purchased => ("外购", "status-purple"),
 AcquireChannel::Outsourced => ("委外", "status-warn"),
 AcquireChannel::NonInventory => ("非库存", "status-muted"),
 }
}

struct ContactInfo {
 name: String,
 phone: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_detail(
 path: OrderDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();
 let customer_svc = state.customer_service();
 let product_svc = state.product_service();
 let user_svc = state.user_service();

 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();
 // 履行计划
 let plan_lines = svc.list_fulfillment_plan(
 &service_ctx, &mut conn,
 FulfillmentPlanQuery { order_id: Some(path.id), status: None },
 ).await.unwrap_or_default();

 // 查询各产品当前可用库存（ATP），用于实时计算满足率
 let stock_svc = state.stock_ledger_service();
 let mut atp_map: HashMap<i64, Decimal> = HashMap::new();
 for pl in &plan_lines {
 if !atp_map.contains_key(&pl.product_id) {
 if let Ok(atp) = stock_svc.query_available(&service_ctx, &mut conn, pl.product_id, None).await {
 atp_map.insert(pl.product_id, atp);
 }
 }
 }

 // 查询该订单关联的需求池（demand）真实状态，用于「需求状态」列
 // 无 demand → 已满足（库存已锁定，无需补货）；有 demand → 按 demand.status 显示
 let demand_svc = state.sales_demand_service();
 let demands = demand_svc
 .find_by_source(&service_ctx, &mut conn, DocumentType::SalesOrder as i16, path.id)
 .await.unwrap_or_default();

 // Smart Button 统计（参考 Odoo oe_button_box）
 let producing_count = demands.iter().filter(|d| d.acquire_channel == 1).count();
 let purchasing_count = demands.iter().filter(|d| d.acquire_channel == 2).count();
 let cascade_count = demands.iter().filter(|d| d.demand_type == 2).count();

 let demand_map: HashMap<i64, DemandStatus> = demands
 .into_iter()
 .map(|d| (d.source_line_id, d.status))
 .collect();

 let customer_name = customer_svc
 .get(&service_ctx, &mut conn, order.customer_id)
 .await
 .map(|c| c.name)
 .unwrap_or_else(|_| "未知客户".into());

 let contact = {
 let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, order.customer_id).await.unwrap_or_default();
 contacts.into_iter().find(|c| c.id == order.contact_id).map(|c| ContactInfo {
 name: c.name,
 phone: c.phone,
 })
 };

 let sales_rep = user_svc
 .get_user(&service_ctx, &mut conn, order.sales_rep_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // 产品信息
 let (product_names, product_codes) = {
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 if product_ids.is_empty() {
 (HashMap::new(), HashMap::new())
 } else {
 let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
 let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
 let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
 (names, codes)
 }
 };

 let content = order_detail_page(
 &order, &items, &plan_lines,
 &customer_name, &contact, &sales_rep,
 &product_names, &product_codes, &atp_map, &demand_map,
 producing_count, purchasing_count, cascade_count, path.id,
 );
 let page_html = admin_page(
 is_htmx, "订单详情", &claims, "sales",
 &format!("{}/{}", OrderListPath::PATH, path.id),
 "销售管理", Some("订单详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_order(
 path: ConfirmOrderPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 svc.confirm(&service_ctx, &mut conn, path.id).await?;

 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn complete_order(
 path: CompleteOrderPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 svc.complete(&service_ctx, &mut conn, path.id).await?;

 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn cancel_order(
 path: CancelOrderPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 svc.cancel(&service_ctx, &mut conn, path.id).await?;

 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: SalesOrderStatus) -> Markup {
 let steps: &[(&str, SalesOrderStatus)] = &[
 ("草稿", SalesOrderStatus::Draft),
 ("已确认", SalesOrderStatus::Confirmed),
 ("部分发货", SalesOrderStatus::PartiallyShipped),
 ("已发货", SalesOrderStatus::Shipped),
 ("已完成", SalesOrderStatus::Completed),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == SalesOrderStatus::Cancelled;

 html! {
 div class="flex items-center mb-6" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
            div class=(format!("w-[48px] h-[2px] {}", if i <= current_idx && !is_cancelled { "bg-success" } else { "bg-border" })) {}
 }
 @let (dot_cls, text_cls, ring_cls) = if is_cancelled {
 ("bg-border-soft", "text-muted", "")
 } else if i < current_idx {
 ("bg-success", "text-success", "")
 } else if i == current_idx {
 ("bg-accent", "text-accent font-semibold", "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]")
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

// ── Fulfillment Progress Bar ──

fn fulfillment_progress(items: &[SalesOrderItem], plan_lines: &[FulfillmentPlanLine]) -> Markup {
    // 加权进度：基于数量（quantity），而非行数（line count）
    let total_qty: Decimal = items.iter().map(|i| i.quantity).sum();
    if total_qty <= Decimal::ZERO {
        return html! {};
    }

    let shipped_qty: Decimal = items.iter().map(|i| i.shipped_qty).sum();
    let allocated_qty: Decimal = plan_lines
        .iter()
        .filter(|p| p.status == FulfillmentLineStatus::Allocated)
        .map(|p| p.reserved_qty)
        .sum();
    let producing_qty: Decimal = plan_lines
        .iter()
        .filter(|p| p.status == FulfillmentLineStatus::Producing)
        .map(|p| p.shortage_qty)
        .sum();
    let purchasing_qty: Decimal = plan_lines
        .iter()
        .filter(|p| p.status == FulfillmentLineStatus::Purchasing)
        .map(|p| p.shortage_qty)
        .sum();
    let pending_qty = total_qty - shipped_qty - allocated_qty - producing_qty - purchasing_qty;
    let restock_qty = producing_qty + purchasing_qty; // 补货中 = 生产中 + 采购中

    // 百分比辅助（trim .0 后缀，如 35.0% → 35%）
    let pct_str = |qty: Decimal| -> String {
        let v = (qty / total_qty * DECIMAL_100).round_dp(1);
        let s = v.to_string();
        if s.ends_with(".0") {
            format!("{}%", &s[..s.len() - 2])
        } else {
            format!("{}%", s)
        }
    };
    let pct_style = |qty: Decimal| -> String {
        let v = (qty / total_qty * DECIMAL_100).round_dp(1);
        format!("width:{}%", v)
    };

    html! {
        div class="bg-bg border border-border rounded-md py-5 px-6 mb-5" {
            // Header: 标题 + 4 个统计箱
            div class="flex items-center justify-between mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg" {
                    (icon::chart_bar_icon("w-4 h-4 text-accent"))
                    "履约进度"
                }
                div class="flex gap-6" {
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-success" { (crate::utils::fmt_qty(shipped_qty)) }
                        div class="text-[11px] text-muted mt-0.5" { "已发货" }
                    }
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-accent" { (crate::utils::fmt_qty(allocated_qty)) }
                        div class="text-[11px] text-muted mt-0.5" { "已分配" }
                    }
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-warn" { (crate::utils::fmt_qty(restock_qty)) }
                        div class="text-[11px] text-muted mt-0.5" { "补货中" }
                    }
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(pending_qty)) }
                        div class="text-[11px] text-muted mt-0.5" { "未交量" }
                    }
                }
            }
            // 细进度条（8px 高，无文字）
            div class="flex h-2 rounded overflow-hidden bg-border-soft" {
                @if shipped_qty > Decimal::ZERO {
                    div class="bg-success [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(pct_style(shipped_qty)) {}
                }
                @if allocated_qty > Decimal::ZERO {
                    div class="bg-accent [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(pct_style(allocated_qty)) {}
                }
                @if producing_qty > Decimal::ZERO {
                    div class="bg-warn [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(pct_style(producing_qty)) {}
                }
                @if purchasing_qty > Decimal::ZERO {
                    div class="bg-purple-500 [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(pct_style(purchasing_qty)) {}
                }
                @if pending_qty > Decimal::ZERO {
                    div class="bg-border [transition:width_600ms_cubic-bezier(0.2,0,0,1)]" style=(pct_style(pending_qty)) {}
                }
            }
            // 图例
            div class="flex gap-5 mt-3 flex-wrap" {
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-success" {}
                    (format!("已发货 {}", pct_str(shipped_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-accent" {}
                    (format!("已分配 {}", pct_str(allocated_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-warn" {}
                    (format!("生产中 {}", pct_str(producing_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-purple-500" {}
                    (format!("采购中 {}", pct_str(purchasing_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-border" {}
                    (format!("待处理 {}", pct_str(pending_qty)))
                }
            }
        }
    }
}

// ── Fulfillment Workbench ──
fn fulfillment_workbench(
 plan_lines: &[FulfillmentPlanLine],
 product_names: &HashMap<i64, String>,
 product_codes: &HashMap<i64, String>,
 atp_map: &HashMap<i64, Decimal>,
 demand_map: &HashMap<i64, DemandStatus>,
 order_id: i64,
) -> Markup {
 if plan_lines.is_empty() {
 return html! {};
 }

 // 需求流转统计
 let mut demand_open = 0usize;
 let mut demand_processing = 0usize;
 let mut demand_done = 0usize;
 for pl in plan_lines {
 match pl.status {
 FulfillmentLineStatus::Pending => demand_open += 1,
 FulfillmentLineStatus::Allocated | FulfillmentLineStatus::Producing | FulfillmentLineStatus::Purchasing => demand_processing += 1,
 FulfillmentLineStatus::Fulfilled => demand_done += 1,
 }
 }
 let demand_total = plan_lines.len();

 html! {
 div class="bg-bg border border-border rounded-md mt-5 overflow-hidden" {
 // ── Header: 标题+badge 在左，操作按钮在右 ──
 div class="flex items-center justify-between p-4 px-5 border-b border-border-soft bg-bg" {
 div class="flex items-center gap-3" {
 span class="text-sm font-semibold text-fg" { "履约工作台" }
 span class="bg-accent-bg text-accent rounded-full text-[11px] font-medium px-2 py-0.5" { (format!("{} 行", demand_total)) }
 }
 div class="flex gap-2" {
 button class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-white text-fg-2 border border-border hover:border-accent hover:text-accent font-medium cursor-pointer transition-all duration-150" {
 (icon::refresh_icon("w-3.5 h-3.5"))
 "刷新状态"
 }
 a class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-white text-fg-2 border border-border hover:border-accent hover:text-accent font-medium cursor-pointer transition-all duration-150" href="/admin/mes/demand-pool" title="生产需求池" {
 (icon::grid_icon("w-3.5 h-3.5"))
 "生产需求池"
 }
 a class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-white text-fg-2 border border-border hover:border-accent hover:text-accent font-medium cursor-pointer transition-all duration-150" href=(format!("/admin/purchase/demand-pool?order_id={}", order_id)) title="查看本订单的采购需求" {
 (icon::clipboard_document_icon("w-3.5 h-3.5"))
 "采购需求池"
 }
 button class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150" {
 (icon::truck_icon("w-3.5 h-3.5"))
 "创建发货单"
 }
 }
 }

 // ── 需求流转状态卡片 ──
 div class="flex gap-3 p-4 flex-wrap" {
 div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center" {
 div class="text-[11px] text-muted mb-1" { "需求总数" }
 div class="text-[22px] font-bold font-mono tabular-nums text-fg" { (demand_total) }
 }
 div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center" {
 div class="text-[11px] text-muted mb-1" { "待处理" }
 div class="text-[22px] font-bold font-mono tabular-nums text-fg" { (demand_open) }
 }
 div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center" {
 div class="text-[11px] text-muted mb-1" { "处理中" }
 div class="text-[22px] font-bold font-mono tabular-nums text-warn" { (demand_processing) }
 }
 div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center" {
 div class="text-[11px] text-muted mb-1" { "已完成" }
 div class="text-[22px] font-bold font-mono tabular-nums text-success" { (demand_done) }
 }
 }

 table class="data-table mb-6" {
 thead {
 tr {
 th { "产品" }
 th { "获取途径" }
 th class="text-right text-[13px]" { "需求量" }
 th class="text-right text-[13px]" { "可满足量" }
 th class="text-right text-[13px]" { "缺口" }
 th { "库存满足率" }
 th { "需求状态" }
 th { "履约状态" }
 th { "下游单据" }
 }
 }
 tbody {
 @for pl in plan_lines {
 (fulfill_plan_row(pl, product_names, product_codes, atp_map, demand_map))
 }
 }
 }
 }
 }
}

fn fulfill_plan_row(
 pl: &FulfillmentPlanLine,
 names: &HashMap<i64, String>,
 codes: &HashMap<i64, String>,
 atp_map: &HashMap<i64, Decimal>,
 demand_map: &HashMap<i64, DemandStatus>,
) -> Markup {
 let p_name = names.get(&pl.product_id).map(|s| s.as_str()).unwrap_or("—");
 let p_code = codes.get(&pl.product_id).map(|s| s.as_str()).unwrap_or("—");
 let (ch_label, ch_class) = acquire_tag(pl.acquire_channel);
 let (st_label, st_class) = fulfill_status_pill(pl.status);

 // 需求状态 — 来自 demand 表的真实需求池状态（不再复用 fulfillment status）
 // 无 demand = 库存已满足（shortage=0，无需补货）；有 demand 则按 demand.status 显示
 let (demand_label, demand_style) = match demand_map.get(&pl.order_line_id) {
 None => ("✓ 已满足", "background:#d1fae5;color:#065f46;"),
 Some(DemandStatus::Pending) => ("⚠ 待补货", "background:#e5e7eb;color:#374151;"),
 Some(DemandStatus::Confirmed) => ("● 已确认", "background:#dbeafe;color:#1e40af;"),
 Some(DemandStatus::InProgress) => ("◐ 补货中", "background:#fef3c7;color:#92400e;"),
 Some(DemandStatus::Fulfilled) => ("✓ 补货完成", "background:#d1fae5;color:#065f46;"),
 Some(DemandStatus::Rejected) => ("✗ 已驳回", "background:#fee2e2;color:#991b1b;"),
 };

 // 满足率（含当前可用库存 ATP，实时反映入库后的库存变化）
 let current_atp = atp_map.get(&pl.product_id).copied().unwrap_or(Decimal::ZERO);
 let effective_qty = (pl.reserved_qty + current_atp).min(pl.required_qty);
 let effective_shortage = (pl.required_qty - effective_qty).max(Decimal::ZERO);
 let fill_pct_val = if pl.required_qty > Decimal::ZERO {
 (effective_qty / pl.required_qty * DECIMAL_100)
 .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
 } else {
 Decimal::ZERO
 };
 let fill_bar_pct = format!("width:{}%", fill_pct_val);
 let fill_pct_str = format!("{}%", fill_pct_val);
 let fill_color = if effective_qty >= pl.required_qty {
 "#10b981"
 } else if effective_qty > Decimal::ZERO {
 "#f59e0b"
 } else {
 "#ef4444"
 };

 // 下游单据链接
 let downstream_doc = match (pl.source_doc_type, pl.source_doc_id) {
 (Some(12), Some(doc_id)) => {
 // ProductionPlan
 Some(html! {
 a href=(format!("/admin/mes/plans/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" class="text-xs" {
 (format!("PP-{}", doc_id))
 }
 })
 }
 (Some(7), Some(doc_id)) => {
 // PurchaseOrder
 Some(html! {
 a href=(format!("/admin/purchase/orders/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" class="text-xs" {
 (format!("PO-{}", doc_id))
 }
 })
 }
 (Some(10), Some(doc_id)) => {
 // WorkOrder
 Some(html! {
 a href=(format!("/admin/mes/orders/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" class="text-xs" {
 (format!("WO-{}", doc_id))
 }
 })
 }
 (Some(11), Some(doc_id)) => {
 // OutsourcingOrder
 Some(html! {
 a href=(format!("/admin/om/outsourcing/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" class="text-xs" {
 (format!("OM-{}", doc_id))
 }
 })
 }
 _ => None,
 };

 html! {
 tr class=(if effective_shortage > Decimal::ZERO { "text-danger" } else if pl.reserved_qty > Decimal::ZERO { "text-warn" } else { "" }) {
 td {
 div {
 span class="block font-medium text-fg text-sm" { (p_name) }
 span class="block text-xs text-muted mt-0.5 font-mono tabular-nums" { (p_code) }
 }
 }
 td {
 span class=(format!("status-pill {}", crate::utils::status_color(ch_class))) { (ch_label) }
 }
 td class="text-right text-[13px]" { (fmt_qty(pl.required_qty)) }
 td class="text-right text-[13px]" { (fmt_qty(effective_qty)) }
 td class="text-right text-[13px]" {
 @if effective_shortage > Decimal::ZERO {
 span class="text-danger" { (fmt_qty(effective_shortage)) }
 } @else {
 span class="text-success" { "0" }
 }
 }
 td {
 div class="flex items-center" class="gap-2" {
 div class="flex-1 overflow-hidden" style="background:#e5e7eb;height:6px;border-radius:3px" {
 div style=(format!("width:{};background:{};height:100%;", fill_bar_pct, fill_color)) {}
 }
 span class="text-xs text-muted" { (fill_pct_str) }
 }
 }
 td {
 span style=(format!("padding:2px 8px;border-radius:12px;font-size:12px;{}", demand_style)) { (demand_label) }
 }
 td {
 span class=(format!("status-pill {}", crate::utils::status_color(st_class))) { (st_label) }
 }
 td {
 @if let Some(doc) = downstream_doc {
 (doc)
 } @else {
 span class="text-muted" { "—" }
 }
 }
 }
 }
}

fn order_detail_page(
 o: &SalesOrder,
 items: &[SalesOrderItem],
 plan_lines: &[FulfillmentPlanLine],
 customer_name: &str,
 contact: &Option<ContactInfo>,
 sales_rep: &str,
 product_names: &HashMap<i64, String>,
 product_codes: &HashMap<i64, String>,
 atp_map: &HashMap<i64, Decimal>,
 demand_map: &HashMap<i64, DemandStatus>,
 producing_count: usize,
 purchasing_count: usize,
 cascade_count: usize,
 order_id: i64,
) -> Markup {
 let (status_text, status_class) = status_label(o.status);
 let contact_name = contact.as_ref().map(|c| c.name.as_str()).unwrap_or("—");
 let contact_phone = contact.as_ref().and_then(|c| c.phone.as_deref()).unwrap_or("—");
 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-1 text-sm text-muted hover:text-accent transition-colors mb-4 icon:w-4 icon:h-4" href=(format!("{}?restore=true", OrderListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回销售订单列表"
 }

 // ── Detail Header (flex layout, matching prototype) ──
 div class="flex items-start justify-between mb-6" {
 div class="flex items-center gap-3" {
 h1 class="text-xl font-bold font-mono tabular-nums text-fg" { (o.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 div class="flex gap-2" {
 button class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::printer_icon("w-4 h-4"))
 "打印"
 }
 @if matches!(o.status, SalesOrderStatus::Confirmed | SalesOrderStatus::PartiallyShipped) {
 a class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href="#" {
 (icon::truck_icon("w-4 h-4"))
 "创建发货申请"
 }
 }
 @if o.status == SalesOrderStatus::Draft {
 button class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(ConfirmOrderPath { id: o.id }.to_string())
 hx-confirm="确认审核此订单？" { "确认订单" }
 }
 @if matches!(o.status, SalesOrderStatus::Draft | SalesOrderStatus::Confirmed) {
 button class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-danger-bg text-danger border border-[rgba(207,19,34,0.2)] hover:bg-danger-100 font-medium cursor-pointer transition-all duration-150"
 hx-post=(CancelOrderPath { id: o.id }.to_string())
 hx-confirm="确认取消此订单？取消后不可恢复。" { "取消订单" }
 }
 }
 }

 // ── Smart Buttons ──
 @if producing_count > 0 || purchasing_count > 0 || cascade_count > 0 {
 div class="flex gap-3 mb-6" {
 @if producing_count > 0 {
 a class="inline-flex items-center gap-2 px-4 py-2 rounded-md border border-border-soft bg-bg shadow-xs hover:shadow-md transition-shadow text-sm"
 href=(format!("/admin/mes/demand-pool?order_id={}", order_id))
 {
 span class="text-lg font-bold text-accent font-mono tabular-nums" { (producing_count) }
 span class="text-muted" { "自制需求" }
 }
 }
 @if purchasing_count > 0 {
 a class="inline-flex items-center gap-2 px-4 py-2 rounded-md border border-border-soft bg-bg shadow-xs hover:shadow-md transition-shadow text-sm"
 href=(format!("/admin/purchase/demand-pool?order_id={}", order_id))
 {
 span class="text-lg font-bold text-warn font-mono tabular-nums" { (purchasing_count) }
 span class="text-muted" { "采购需求" }
 }
 }
 @if cascade_count > 0 {
 a class="inline-flex items-center gap-2 px-4 py-2 rounded-md border border-border-soft bg-bg shadow-xs hover:shadow-md transition-shadow text-sm"
 href=(format!("/admin/purchase/demand-pool?order_id={}", order_id))
 {
 span class="text-lg font-bold text-purple font-mono tabular-nums" { (cascade_count) }
 span class="text-muted" { "BOM展开需求" }
 }
 }
 }
 }
 // ── Workflow Steps ──
 (workflow_steps(o.status))

 // ── Fulfillment Progress ──
 (fulfillment_progress(items, plan_lines))

 // ── Order Info ──
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-5 shadow-[var(--shadow-card)]" {
 div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "订单信息" }
 div class="grid grid-cols-2 md:grid-cols-3 gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "客户名称" }
 span class="text-sm text-fg font-medium" { (customer_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "联系人" }
 span class="text-sm text-fg font-medium" { (contact_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "联系电话" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (contact_phone) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "业务员" }
 span class="text-sm text-fg font-medium" { (sales_rep) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "交货日期" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (o.order_date.format("%Y-%m-%d")) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "付款条款" }
 span class="text-sm text-fg font-medium" { (o.payment_terms.as_str()) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "交货条款" }
 span class="text-sm text-fg font-medium" { (o.delivery_terms.as_str()) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "交货地址" }
 span class="text-sm text-fg font-medium" { (o.delivery_address.as_str()) }
 }
 }
 }

 // ── Items Table (四量模型) ──
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "行号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "单位" }
 th class="text-right text-[13px]" { "订单量" }
 th class="text-right text-[13px]" { "已发货" }
 th class="text-right text-[13px]" { "已取消" }
 th class="text-right text-[13px]" { "未交量" }
 th class="text-right text-[13px]" { "单价" }
 th class="text-right text-[13px]" { "小计" }
 th { "行状态" }
 th { "交货日期" }
 }
 }
 tbody {
 @for item in items {
 (item_row(item, product_names, product_codes))
 }
 @if items.is_empty() {
 tr {
 td colspan="12" class="text-center p-8 text-muted" {
 "暂无明细"
 }
 }
 }
 }
 }
 }
 div class="flex justify-end gap-8 p-5 border-t border-border-soft bg-surface-raised" {
 div class="flex gap-3" {
 span class="text-[11px] text-muted font-medium uppercase" { "成本合计" }
 span class="text-[20px] font-bold text-fg" { (crate::utils::fmt_amount(o.total_cost)) }
 }
 div class="flex gap-3" {
 span class="text-[11px] text-muted font-medium uppercase" { "订单总额" }
 span class="text-[20px] font-bold text-fg accent" { (crate::utils::fmt_amount(o.total_amount)) }
 }
 }
 }

 // ── Fulfillment Workbench ──
 (fulfillment_workbench(plan_lines, product_names, product_codes, atp_map, demand_map, order_id))

 // ── Remarks ──
 @if !o.remark.is_empty() {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" class="mt-6" {
 div class="text-sm font-semibold text-fg mb-4" { "备注" }
 p class="text-muted" { (o.remark.as_str()) }
 }
 }
 }
 }
}

fn item_row(
 item: &SalesOrderItem,
 names: &HashMap<i64, String>,
 codes: &HashMap<i64, String>,
) -> Markup {
 let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
 let open_qty = item.open_qty();
 let (ls_label, ls_class) = line_status_pill(item.line_status);

 html! {
 tr {
 td class="font-mono tabular-nums" { (item.line_no) }
 td class="font-mono tabular-nums" { (product_code) }
 td { (product_name) }
 td { (item.unit.as_str()) }
 td class="text-right text-[13px]" { (fmt_qty(item.quantity)) }
 td class="text-right text-[13px]" { (fmt_qty(item.shipped_qty)) }
 td class="text-right text-[13px]" { (fmt_qty(item.cancelled_qty)) }
 td class="text-right text-[13px]" {
 @if open_qty > Decimal::ZERO {
 span class="text-danger" { (fmt_qty(open_qty)) }
 } @else {
 (fmt_qty(open_qty))
 }
 }
 td class="text-right text-[13px]" { (crate::utils::fmt_amount(item.unit_price)) }
 td class="text-right text-[13px]" { (crate::utils::fmt_amount(item.amount)) }
 td {
 span class=(format!("status-pill {}", crate::utils::status_color(ls_class))) { (ls_label) }
 }
 td class="font-mono tabular-nums" { (delivery) }
 }
 }
}
