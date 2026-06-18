use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};

use crate::state::AppState;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::sales_return::SalesReturnService;
use abt_core::sales::sales_return::model::*;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PgExecutor;
use abt_core::shared::types::ServiceContext;

use crate::components::icon;
use crate::utils::fmt_qty;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::OrderDetailPath;
use crate::routes::sales_return::*;
use crate::routes::shipping::ShippingDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: ReturnStatus) -> (&'static str, &'static str) {
 match s {
 ReturnStatus::Draft => ("草稿", "status-draft"),
 ReturnStatus::Confirmed => ("已确认", "status-confirmed"),
 ReturnStatus::Received => ("已收货", "status-progress"),
 ReturnStatus::Inspecting => ("质检中", "status-inspecting"),
 ReturnStatus::Completed => ("已完成", "status-completed"),
 ReturnStatus::Cancelled => ("已取消", "status-cancelled"),
 ReturnStatus::Rejected => ("已驳回", "status-rejected"),
 }
}

fn disposition_label(d: ReturnDisposition) -> &'static str {
 match d {
 ReturnDisposition::Restock => "退回库存",
 ReturnDisposition::Scrap => "报废",
 ReturnDisposition::Rework => "返工",
 }
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_return_detail(
 path: ReturnDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 claims,
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 // Fetch return header
 let ret = state
 .sales_return_service()
 .find_by_id(&service_ctx, &mut conn, path.id)
 .await?;

 // Fetch return items
 let items = state
 .sales_return_service()
 .list_items(&service_ctx, &mut conn, path.id)
 .await
 .unwrap_or_default();

 // Resolve customer name
 let customer_name = state
 .customer_service()
 .get(&service_ctx, &mut conn, ret.customer_id)
 .await
 .map(|c| c.name)
 .unwrap_or_else(|_| "未知客户".into());

 // Resolve order number
 let order_number = state
 .sales_order_service()
 .find_by_id(&service_ctx, &mut conn, ret.order_id)
 .await
 .map(|o| o.doc_number)
 .unwrap_or_else(|_| "—".into());

 // Resolve shipping number
 let shipping_number = state
 .shipping_service()
 .find_by_id(&service_ctx, &mut conn, ret.shipping_request_id)
 .await
 .map(|s| s.doc_number)
 .unwrap_or_else(|_| "—".into());

 // Resolve operator name
 let operator_name = state
 .user_service()
 .get_user(&service_ctx, &mut conn, ret.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // Resolve product details
 let product_details = resolve_product_details(&state, &service_ctx, &mut conn, &items).await;

 let content = return_detail_page(
 &ret,
 &items,
 &customer_name,
 &order_number,
 &shipping_number,
 &operator_name,
 &product_details,
 );
 let page_html = admin_page(
 is_htmx,
 "退货详情",
 &claims,
 "sales",
 &format!("{}/{}", ReturnListPath::PATH, path.id),
 "销售管理",
 Some("退货详情"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_return(
 path: ConfirmReturnPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 state
 .sales_return_service()
 .approve(&service_ctx, &mut conn, path.id)
 .await?;

 let redirect = ReturnDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn receive_return(
 path: ReceiveReturnPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 state
 .sales_return_service()
 .receive(&service_ctx, &mut conn, path.id)
 .await?;

 let redirect = ReturnDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn inspect_return(
 path: InspectReturnPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 state
 .sales_return_service()
 .inspect(&service_ctx, &mut conn, path.id)
 .await?;

 let redirect = ReturnDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn complete_return(
 path: CompleteReturnPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 state
 .sales_return_service()
 .complete(&service_ctx, &mut conn, path.id)
 .await?;

 let redirect = ReturnDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn reject_return(
 path: RejectReturnPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 state
 .sales_return_service()
 .reject(&service_ctx, &mut conn, path.id)
 .await?;

 let redirect = ReturnDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Product Detail Resolution ──

struct ProductDetail {
 code: String,
 name: String,
 unit: String,
}

async fn resolve_product_details(
 state: &AppState,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 items: &[SalesReturnItem],
) -> HashMap<i64, ProductDetail> {
 let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 let products = state
 .product_service()
 .get_by_ids(ctx, db, ids)
 .await
 .unwrap_or_default();
 products
 .into_iter()
 .map(|p| {
 (
 p.product_id,
 ProductDetail {
 code: p.product_code,
 name: p.pdt_name,
 unit: p.unit,
 },
 )
 })
 .collect()
}

// ── Workflow Steps ──

fn workflow_steps(current: ReturnStatus) -> Markup {
 let steps: &[(&str, ReturnStatus)] = &[
 ("草稿", ReturnStatus::Draft),
 ("已确认", ReturnStatus::Confirmed),
 ("已收货", ReturnStatus::Received),
 ("质检中", ReturnStatus::Inspecting),
 ("已完成", ReturnStatus::Completed),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == ReturnStatus::Cancelled;
 let is_rejected = current == ReturnStatus::Rejected;
 let terminal = is_cancelled || is_rejected;

 html! {
 div class="flex items-center mt-6 mb-6" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
 div class=(format!("w-[48px] h-[2px] {}", if i <= current_idx && !terminal { "bg-[#10b981]" } else { "bg-border" })) {}
 }
 @let (dot_cls, text_cls, ring_cls) = if terminal {
 ("bg-border-soft", "text-muted", "")
 } else if i < current_idx {
 ("bg-[#10b981]", "text-[#10b981]", "")
 } else if i == current_idx {
 ("bg-[#2563eb]", "text-[#2563eb] font-semibold", "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]")
 } else {
 ("bg-[#d1d5db]", "text-[#9ca3af]", "")
 };
 div class="flex items-center gap-2 shrink-0" {
 span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
 span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
 }
 }
 @if is_cancelled {
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 shrink-0" {
 span class="w-2.5 h-2.5 rounded-full shrink-0 bg-[#ef4444]" {}
 span class="text-xs text-[#ef4444] font-semibold whitespace-nowrap" { "已取消" }
 }
 }
 @if is_rejected {
 div class="w-[48px] h-[2px] bg-border" {}
 div class="flex items-center gap-2 shrink-0" {
 span class="w-2.5 h-2.5 rounded-full shrink-0 bg-[#ef4444]" {}
 span class="text-xs text-[#ef4444] font-semibold whitespace-nowrap" { "已驳回" }
 }
 }
 }
 }
}

// ── Components ──

fn return_detail_page(
 r: &SalesReturn,
 items: &[SalesReturnItem],
 customer_name: &str,
 order_number: &str,
 shipping_number: &str,
 operator_name: &str,
 product_details: &HashMap<i64, ProductDetail>,
) -> Markup {
 let (status_text, status_class) = status_label(r.status);
 let shipping_detail = ShippingDetailPath {
 id: r.shipping_request_id,
 };
 let order_detail = OrderDetailPath { id: r.order_id };

 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ReturnListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回退货列表"
 }
 // ── Detail Header ──
 div class="block bg-bg border border-border-soft rounded-lg p-6" {
 div {
 div class="flex items-center justify-between" {
 h1 class="text-2xl font-extrabold font-mono tabular-nums" { (r.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 div class="text-[13px] text-muted" {
 "来源发货："
 a href=(shipping_detail.to_string()) { (shipping_number) }
 "　来源订单："
 a href=(order_detail.to_string()) {
 (order_number)
 }
 }
 }
 div class="flex gap-3" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", ReturnListPath::PATH)) { "返回列表" }
 @if r.status == ReturnStatus::Draft {
 button
 class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(ConfirmReturnPath { id: r.id }.to_string())
 hx-confirm="确认审核此退货单？"
 { "确认退货" }
 }
 @if r.status == ReturnStatus::Confirmed {
 button
 class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(ReceiveReturnPath { id: r.id }.to_string())
 hx-confirm="确认已收到退货？"
 { "确认收货" }
 }
 @if r.status == ReturnStatus::Received {
 button
 class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(InspectReturnPath { id: r.id }.to_string())
 hx-confirm="确认开始质检？"
 { "开始质检" }
 }
 @if r.status == ReturnStatus::Inspecting {
 button
 class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-[#10b981] text-[#fff]"
 hx-post=(CompleteReturnPath { id: r.id }.to_string())
 hx-confirm="确认完成退货？"
 { "完成退货" }
 button
 class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
 hx-post=(RejectReturnPath { id: r.id }.to_string())
 hx-confirm="确认驳回此退货？"
 { "驳回" }
 }
 }
 }
 // ── Workflow Steps ──
 (workflow_steps(r.status))
 // ── Return Info ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "退货信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "客户名称" }
 span class="text-sm text-fg font-medium" { (customer_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "退货原因" }
 span class="text-sm text-fg font-medium" { (r.return_reason.as_str()) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "操作员" }
 span class="text-sm text-fg font-medium" { (operator_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "创建时间" }
 span class="text-sm text-fg font-medium" { (r.created_at.format("%Y-%m-%d %H:%M")) }
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
 th { "单位" }
 th class="text-right text-[13px]" { "单价" }
 th class="text-right text-[13px]" { "退货数量" }
 th class="text-right text-[13px]" { "退货金额" }
 th { "处理方式" }
 }
 }
 tbody {
 @for (i, item) in items.iter().enumerate() {
 (item_row(i, item, product_details))
 }
 @if items.is_empty() {
 tr {
 td colspan="8" class="text-center p-8 text-muted"
 { "暂无明细" }
 }
 }
 }
 }
 }
 div class="flex justify-end gap-8 p-5 [border-top:1px_solid_var(--border-soft)] bg-surface-raised" {
 div class="flex gap-3" {
 span { "退货总额" }
 span class="mono text-2xl font-bold font-mono tabular-nums tabular-nums text-fg-lg" {
 "¥ "
 (format!("{:.2}", r.total_amount))
 }
 }
 }
 }
 // ── Remarks ──
 @if !r.remark.is_empty() {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] mt-6" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "备注" }
 p class="text-muted" { (r.remark.as_str()) }
 }
 }
 }
 }
}

fn item_row(index: usize, item: &SalesReturnItem, details: &HashMap<i64, ProductDetail>) -> Markup {
 let detail = details.get(&item.product_id);
 let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
 let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
 let unit = detail.map(|d| d.unit.as_str()).unwrap_or("—");

 html! {
 tr {
 td class="font-mono tabular-nums" { (index + 1) }
 td class="font-mono tabular-nums" { (product_code) }
 td { (product_name) }
 td { (unit) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.unit_price)) }
 td class="text-right text-[13px]" { (fmt_qty(item.returned_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.amount)) }
 td { (disposition_label(item.disposition)) }
 }
 }
}
