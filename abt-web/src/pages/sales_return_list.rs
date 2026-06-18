use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::sales_return::model::*;
use abt_core::sales::sales_return::SalesReturnService;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::{PageParams, PgExecutor, ServiceContext};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::OrderDetailPath;
use crate::routes::sales_return::*;
use crate::routes::shipping::ShippingDetailPath;
use crate::utils::{empty_as_none, resolve_customer_names, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReturnQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub customer_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn build_query_string(params: &ReturnQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref kw) = params.keyword {
 q.push(format!("keyword={kw}"));
 }
 if let Some(s) = params.status {
 q.push(format!("status={s}"));
 }
 if let Some(c) = params.customer_id {
 q.push(format!("customer_id={c}"));
 }
 q.join("&")
}

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

/// Compute status counts by calling SalesReturnService::list for each status with page_size=1.
/// Returns a map of status i16 -> total count.
async fn count_by_status<S: SalesReturnService>(
 svc: &S,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 customer_id: Option<i64>,
) -> HashMap<i16, u64> {
 let statuses = [
 (ReturnStatus::Draft, 1i16),
 (ReturnStatus::Confirmed, 2),
 (ReturnStatus::Received, 3),
 (ReturnStatus::Inspecting, 4),
 (ReturnStatus::Completed, 5),
 (ReturnStatus::Rejected, 7),
 ];

 let mut counts = HashMap::new();
 for (status, code) in statuses {
 let filter = ReturnQuery {
 customer_id,
 status: Some(status),
 ..Default::default()
 };
 let page = PageParams::new(1, 1);
 if let Ok(result) = svc.list(ctx, db, filter, page).await {
 counts.insert(code, result.total);
 }
 }

 // Total = sum of all per-status counts
 let total: u64 = counts.values().sum();
 counts.insert(0, total);

 counts
}

/// Resolve shipping doc_numbers by calling ShippingRequestService::find_by_id for each unique id.
async fn resolve_shipping_numbers<S: ShippingRequestService>(
 svc: &S,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 items: &[SalesReturn],
) -> HashMap<i64, String> {
 let mut map = HashMap::new();
 let mut seen = std::collections::HashSet::new();
 for item in items {
 if seen.insert(item.shipping_request_id)
 && let Ok(shipping) = svc.find_by_id(ctx, db, item.shipping_request_id).await {
 map.insert(item.shipping_request_id, shipping.doc_number);
 }
 }
 map
}

/// Resolve order doc_numbers by calling SalesOrderService::find_by_id for each unique id.
async fn resolve_order_numbers<S: SalesOrderService>(
 svc: &S,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 items: &[SalesReturn],
) -> HashMap<i64, String> {
 let mut map = HashMap::new();
 let mut seen = std::collections::HashSet::new();
 for item in items {
 if seen.insert(item.order_id)
 && let Ok(order) = svc.find_by_id(ctx, db, item.order_id).await {
 map.insert(item.order_id, order.doc_number);
 }
 }
 map
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_return_list(
 _path: ReturnListPath,
 ctx: RequestContext,
 Query(params): Query<ReturnQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("SHIPPING", "create").await;
 let can_delete = ctx.has_permission("SHIPPING", "delete").await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let return_svc = state.sales_return_service();
 let customer_svc = state.customer_service();
 let shipping_svc = state.shipping_service();
 let order_svc = state.sales_order_service();

 let filter = ReturnQuery {
 order_id: None,
 shipping_request_id: None,
 customer_id: params.customer_id,
 status: params.status.and_then(ReturnStatus::from_i16),
 keyword: params.keyword.clone(),
 };
 let page = PageParams::new(params.page.unwrap_or(1), 20);
 let result = return_svc.list(&service_ctx, &mut conn, filter, page).await?;

 let status_counts = count_by_status(&return_svc, &service_ctx, &mut conn, params.customer_id).await;
 let customer_names = resolve_customer_names(&customer_svc, &service_ctx, &mut conn, result.items.iter().map(|i| i.customer_id)).await;
 let shipping_numbers = resolve_shipping_numbers(&shipping_svc, &service_ctx, &mut conn, &result.items).await;
 let order_numbers = resolve_order_numbers(&order_svc, &service_ctx, &mut conn, &result.items).await;

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let content = return_list_page(&claims, &result, &customer_names, &shipping_numbers, &order_numbers, &customers.items, &params, &status_counts, can_create, can_delete);
 let page_html = admin_page(
 is_htmx, "销售退货", &claims, "sales", ReturnListPath::PATH, "销售管理", Some("销售退货"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "delete")]
pub async fn delete_return(
 path: ReturnDeletePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let return_svc = state.sales_return_service();
 return_svc.delete(&service_ctx, &mut conn, path.id).await?;

 let redirect = ReturnListPath::PATH.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

#[allow(clippy::too_many_arguments)]
fn return_list_page(
 _claims: &abt_core::shared::identity::model::Claims,
 result: &abt_core::shared::types::PaginatedResult<SalesReturn>,
 customer_names: &std::collections::HashMap<i64, String>,
 shipping_numbers: &std::collections::HashMap<i64, String>,
 order_numbers: &std::collections::HashMap<i64, String>,
 customers: &[abt_core::master_data::customer::model::Customer],
 params: &ReturnQueryParams,
 status_counts: &HashMap<i16, u64>,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "销售退货" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ReturnCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建退货单"
 }
 }
 }
 }
 (return_table_fragment(result, customer_names, shipping_numbers, order_numbers, customers, params, status_counts, can_delete))
 }
 }
}

fn return_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<SalesReturn>,
 customer_names: &std::collections::HashMap<i64, String>,
 shipping_numbers: &std::collections::HashMap<i64, String>,
 order_numbers: &std::collections::HashMap<i64, String>,
 customers: &[abt_core::master_data::customer::model::Customer],
 params: &ReturnQueryParams,
 status_counts: &HashMap<i16, u64>,
 can_delete: bool,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();

 let total_count: u64 = status_counts.get(&0).copied().unwrap_or_default();
 let draft_count = status_counts.get(&1).copied();
 let confirmed_count = status_counts.get(&2).copied();
 let received_count = status_counts.get(&3).copied();
 let inspecting_count = status_counts.get(&4).copied();
 let completed_count = status_counts.get(&5).copied();
 let cancelled_count = status_counts.get(&6).copied();
 let rejected_count = status_counts.get(&7).copied();

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: draft_count },
 TabItem { value: "2".into(), label: "已确认", count: confirmed_count },
 TabItem { value: "3".into(), label: "已收货", count: received_count },
 TabItem { value: "4".into(), label: "质检中", count: inspecting_count },
 TabItem { value: "5".into(), label: "已完成", count: completed_count },
 TabItem { value: "6".into(), label: "已取消", count: cancelled_count },
 TabItem { value: "7".into(), label: "已驳回", count: rejected_count },
 ];

 let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();

 html! {
 div class="return-list-panel" {
 (status_tabs_with_param(ReturnListPath::PATH, "#return-data-card", "#return-filter-form", tabs, &active_value, "status"))

 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="return-filter-form"
 hx-get=(ReturnListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#return-data-card"
 hx-select="#return-data-card"
 hx-swap="outerHTML"
 hx-select-oob="#status-tabs"
 hx-include="#return-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="keyword"
 placeholder="搜索退货单号、客户名称…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="customer_id" {
 option value="" { "全部客户" }
 @for c in customers {
 option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
 }
 }
 }

 div class="data-card" id="return-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "退货单号" }
 th { "来源发货" }
 th { "来源订单" }
 th { "客户名称" }
 th { "状态" }
 th class="text-right text-[13px]" { "退货金额" }
 th { "退货原因" }
 th { "创建时间" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for r in &result.items {
 (return_row(r, customer_names, shipping_numbers, order_numbers, can_delete))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="9" class="text-center p-8 text-muted" {
 "暂无退货数据"
 }
 }
 }
 }
 }
 }
 (pagination(ReturnListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
 }
}

fn return_row(
 r: &SalesReturn,
 customer_names: &std::collections::HashMap<i64, String>,
 shipping_numbers: &std::collections::HashMap<i64, String>,
 order_numbers: &std::collections::HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 let detail_path = ReturnDetailPath { id: r.id };
 let (status_text, status_class) = status_label(r.status);
 let customer_name = customer_names.get(&r.customer_id).map(|n| n.as_str()).unwrap_or("—");
 let shipping_num = shipping_numbers.get(&r.shipping_request_id).map(|n| n.as_str()).unwrap_or("—");
 let order_num = order_numbers.get(&r.order_id).map(|n| n.as_str()).unwrap_or("—");
 let created = r.created_at.format("%Y-%m-%d %H:%M").to_string();
 let onclick = format!("location.href='{}'", detail_path);
 let is_draft = r.status == ReturnStatus::Draft;
 let delete_path = ReturnDeletePath { id: r.id };
 let shipping_detail = ShippingDetailPath { id: r.shipping_request_id };
 let order_detail = OrderDetailPath { id: r.order_id };

 html! {
 tr {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) { (r.doc_number) }
 td onclick=(&onclick) {
 a href=(shipping_detail.to_string()) class="text-accent" _="on click halt the event" { (shipping_num) }
 }
 td onclick=(&onclick) {
 a href=(order_detail.to_string()) class="text-accent" _="on click halt the event" { (order_num) }
 }
 td onclick=(&onclick) { (customer_name) }
 td onclick=(&onclick) {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 td class="text-right text-[13px]" onclick=(&onclick) {
 span class="font-mono tabular-nums" { (crate::utils::fmt_amount(r.total_amount)) }
 }
 td onclick=(&onclick) { (r.return_reason.as_str()) }
 td onclick=(&onclick) { (created) }
 td _="on click halt the event" {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 @if is_draft {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(detail_path.to_string()) title="编辑" {
 (icon::edit_icon("w-4 h-4"))
 }
 @if can_delete {
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
 hx-confirm=(format!("确认删除退货单 {}？", r.doc_number))
 hx-post=(delete_path.to_string())
 hx-target="closest tr"
 hx-swap="outerHTML swap:0.5s" {
 (icon::trash_icon("w-4 h-4"))
 }
 }
 } @else {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(detail_path.to_string()) title="查看详情" {
 (icon::eye_icon("w-4 h-4"))
 }
 }
 }
 }
 }
 }
}
