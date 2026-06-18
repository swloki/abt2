use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;
use abt_core::shared::types::ServiceContext;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::utils::{empty_as_none, resolve_customer_names, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrderQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub customer_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_range: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
 let today = chrono::Local::now().date_naive();
 match range {
 "7d" => (Some(today - chrono::Days::new(7)), None),
 "30d" => (Some(today - chrono::Days::new(30)), None),
 "3m" => (Some(today - chrono::Months::new(3)), None),
 _ => (None, None),
 }
}

fn build_filter(params: &OrderQueryParams) -> SalesOrderQuery {
 let (date_from, date_to) = params
 .date_range
 .as_deref()
 .map(parse_date_range)
 .unwrap_or((None, None));
 SalesOrderQuery {
 keyword: params.keyword.clone(),
 status: params.status.and_then(SalesOrderStatus::from_i16),
 customer_id: params.customer_id,
 date_from,
 date_to,
 }
}

fn build_query_string(params: &OrderQueryParams) -> String {
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
 if let Some(ref dr) = params.date_range {
 q.push(format!("date_range={dr}"));
 }
 q.join("&")
}

async fn resolve_sales_rep_names<S: UserService>(
 svc: &S,
 ctx: &ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 orders: &[SalesOrder],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = orders.iter().map(|o| o.sales_rep_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 svc.get_users_by_ids(ctx, db, ids)
 .await
 .map(|users| {
 users.into_iter()
 .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
 .collect()
 })
 .unwrap_or_default()
}

// ── Status Labels ──

fn status_label(s: SalesOrderStatus) -> (&'static str, &'static str) {
 match s {
 SalesOrderStatus::Draft => ("草稿", "status-draft"),
 SalesOrderStatus::Confirmed => ("已确认", "status-confirmed"),
 SalesOrderStatus::PartiallyShipped => ("部分发货", "status-partial"),
 SalesOrderStatus::Shipped => ("已发货", "status-shipped"),
 SalesOrderStatus::Completed => ("已完成", "status-completed"),
 SalesOrderStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_list(
 _path: OrderListPath,
 ctx: RequestContext,
 Query(params): Query<OrderQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("SALES_ORDER", "create").await;
 let can_delete = ctx.has_permission("SALES_ORDER", "delete").await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();
 let customer_svc = state.customer_service();
 let user_svc = state.user_service();

 let filter = build_filter(&params);
 let page = PageParams::new(params.page.unwrap_or(1), 20);
 let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

 let customer_names = resolve_customer_names(&customer_svc, &service_ctx, &mut conn, result.items.iter().map(|o| o.customer_id)).await;
 let sales_rep_names = resolve_sales_rep_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let content = order_list_page(&result, &customer_names, &sales_rep_names, &customers.items, &params, can_create, can_delete);
 let page_html = admin_page(
 is_htmx, "销售订单", &claims, "sales", OrderListPath::PATH, "销售管理", Some("销售订单"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

// ── Edit / Delete Handlers ──

pub async fn delete_order(
 path: DeleteOrderPath,
 ctx: RequestContext,
) -> Result<impl axum::response::IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 svc.delete(&service_ctx, &mut conn, path.id).await?;

 Ok(([("HX-Redirect", OrderListPath::PATH)], Html(String::new())))
}

// ── Components ──

fn order_list_page(
 result: &abt_core::shared::types::PaginatedResult<SalesOrder>,
 customer_names: &HashMap<i64, String>,
 sales_rep_names: &HashMap<i64, String>,
 customers: &[abt_core::master_data::customer::model::Customer],
 params: &OrderQueryParams,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "销售订单" }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::download_icon("w-4 h-4"))
 "导出"
 }
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(OrderCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建订单"
 }
 }
 }
 }

 // ── Tabs + Filter + Data Table (HTMX panel) ──
 (order_table_fragment(result, customer_names, sales_rep_names, customers, params, can_delete))
 }
 }
}

fn order_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<SalesOrder>,
 customer_names: &HashMap<i64, String>,
 sales_rep_names: &HashMap<i64, String>,
 customers: &[abt_core::master_data::customer::model::Customer],
 params: &OrderQueryParams,
 can_delete: bool,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: None },
 TabItem { value: "2".into(), label: "已确认", count: None },
 TabItem { value: "3".into(), label: "生产中", count: None },
 TabItem { value: "4".into(), label: "部分发货", count: None },
 TabItem { value: "5".into(), label: "已发货", count: None },
 TabItem { value: "6".into(), label: "已完成", count: None },
 TabItem { value: "7".into(), label: "已取消", count: None },
 ];

 let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();
 let selected_range = params.date_range.as_deref().unwrap_or("");

 html! {
 div class="order-list-panel" {
 (status_tabs_with_param(OrderListPath::PATH, "#order-data-card", "#order-filter-form", tabs, &active_value, "status"))

 // ── Filter Bar ──
 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="order-filter-form"
 hx-get=(OrderListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#order-data-card"
 hx-select="#order-data-card"
 hx-swap="outerHTML"
 hx-select-oob="#status-tabs"
 hx-include="#order-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
 placeholder="搜索订单号、客户名称…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="customer_id" {
 option value="" { "全部客户" }
 @for c in customers {
 option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
 }
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="date_range" {
 option value="" selected[selected_range.is_empty()] { "交货日期" }
 option value="7d" selected[selected_range == "7d"] { "最近7天" }
 option value="30d" selected[selected_range == "30d"] { "最近30天" }
 option value="3m" selected[selected_range == "3m"] { "最近3个月" }
 }
 }

 // ── Data Table ──
 div class="data-card" id="order-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "订单号" }
 th { "来源报价" }
 th { "客户名称" }
 th { "状态" }
 th class="text-right text-[13px]" { "总金额" }
 th { "交货日期" }
 th { "业务员" }
 th { "创建时间" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for o in &result.items {
 (order_row(o, customer_names, sales_rep_names, can_delete))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="9" class="text-center p-8 text-muted" {
 "暂无订单数据"
 }
 }
 }
 }
 }
 }
 (pagination(OrderListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
 }
}

fn order_row(
 o: &SalesOrder,
 customer_names: &HashMap<i64, String>,
 sales_rep_names: &HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 let detail_path = OrderDetailPath { id: o.id };
 let edit_form_path = OrderEditFormPath { id: o.id };
 let delete_path = DeleteOrderPath { id: o.id };
 let (status_text, status_class) = status_label(o.status);
 let customer_name = customer_names.get(&o.customer_id).map(|s| s.as_str()).unwrap_or("—");
 let sales_rep = sales_rep_names.get(&o.sales_rep_id).map(|s| s.as_str()).unwrap_or("—");
 let created = o.created_at.format("%Y-%m-%d").to_string();
 let onclick = format!("location.href='{}'", detail_path);
 let is_draft = o.status == SalesOrderStatus::Draft;

 html! {
 tr {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) { (o.doc_number) }
 td onclick=(&onclick) { "—" }
 td onclick=(&onclick) { (customer_name) }
 td onclick=(&onclick) {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 td class="text-right text-[13px]" onclick=(&onclick) {
 span class="font-mono tabular-nums" { (crate::utils::fmt_amount(o.total_amount)) }
 }
 td class="font-mono tabular-nums" onclick=(&onclick) { (o.order_date.format("%Y-%m-%d")) }
 td onclick=(&onclick) { (sales_rep) }
 td onclick=(&onclick) { (created) }
 td _="on click halt the event" {
 @if is_draft {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(edit_form_path.to_string()) title="编辑" {
 (icon::edit_icon("w-4 h-4"))
 }
 @if can_delete {
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
 hx-confirm="确认删除该订单吗？"
 hx-post=(delete_path)
 hx-target="closest tr"
 hx-swap="outerHTML swap:0.5s" {
 (icon::trash_icon("w-4 h-4"))
 }
 }
 }
 }
 }
 }
 }
}

