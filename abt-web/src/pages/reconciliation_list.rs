use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::reconciliation::model::*;
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::shared::types::{PageParams, ServiceContext};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::utils::{empty_as_none, resolve_customer_names, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReconciliationQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub customer_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub period: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn build_query_string(params: &ReconciliationQueryParams) -> String {
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
 if let Some(ref p) = params.period {
 q.push(format!("period={p}"));
 }
 q.join("&")
}

fn status_label(s: ReconciliationStatus) -> (&'static str, &'static str) {
 match s {
 ReconciliationStatus::Draft => ("草稿", "status-draft"),
 ReconciliationStatus::Sent => ("已发送", "status-sent"),
 ReconciliationStatus::Confirmed => ("已确认", "status-confirmed"),
 ReconciliationStatus::Disputed => ("有异议", "status-disputed"),
 ReconciliationStatus::Settled => ("已结算", "status-settled"),
 }
}

/// Compute status counts by calling ReconciliationService::list for each status with page_size=1.
async fn count_by_status<S: ReconciliationService>(
 svc: &S,
 ctx: &ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 customer_id: Option<i64>,
) -> HashMap<i16, u64> {
 let statuses = [
 (ReconciliationStatus::Draft, 1i16),
 (ReconciliationStatus::Sent, 2),
 (ReconciliationStatus::Confirmed, 3),
 (ReconciliationStatus::Disputed, 4),
 (ReconciliationStatus::Settled, 5),
 ];

 let mut counts = HashMap::new();
 for (status, code) in statuses {
 let filter = ReconciliationQuery {
 customer_id,
 period: None,
 status: Some(status),
 keyword: None,
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

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_reconciliation_list(
 _path: ReconciliationListPath,
 ctx: RequestContext,
 Query(params): Query<ReconciliationQueryParams>,
) -> Result<Html<String>> {
 let can_create = ctx.has_permission("SALES_ORDER", "create").await;
 let can_delete = ctx.has_permission("SALES_ORDER", "delete").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 let customer_svc = state.customer_service();

 let filter = ReconciliationQuery {
 customer_id: params.customer_id,
 period: params.period.clone(),
 status: params.status.and_then(ReconciliationStatus::from_i16),
 keyword: params.keyword.clone(),
 };
 let page = PageParams::new(params.page.unwrap_or(1), 20);
 let result = reconciliation_svc.list(&service_ctx, &mut conn, filter, page).await?;

 let status_counts = count_by_status(&reconciliation_svc, &service_ctx, &mut conn, params.customer_id).await;
 let customer_names = resolve_customer_names(&customer_svc, &service_ctx, &mut conn, result.items.iter().map(|i| i.customer_id)).await;

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let content = reconciliation_list_page(&claims, &result, &customer_names, &customers.items, &params, &status_counts, can_create, can_delete);
 let page_html = admin_page(
 is_htmx, "月对账单", &claims, "sales", ReconciliationListPath::PATH, "销售管理", Some("月对账单"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "delete")]
pub async fn delete_reconciliation(
 path: ReconciliationDeletePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 reconciliation_svc.delete(&service_ctx, &mut conn, path.id).await?;

 let redirect = ReconciliationListPath::PATH.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn reconciliation_list_page(
 _claims: &abt_core::shared::identity::model::Claims,
 result: &abt_core::shared::types::PaginatedResult<Reconciliation>,
 customer_names: &std::collections::HashMap<i64, String>,
 customers: &[abt_core::master_data::customer::model::Customer],
 params: &ReconciliationQueryParams,
 status_counts: &HashMap<i16, u64>,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "月对账单" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ReconciliationCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建对账单"
 }
 }
 }
 }
 (reconciliation_table_fragment(result, customer_names, customers, params, status_counts, can_delete))
 }
 }
}

fn reconciliation_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<Reconciliation>,
 customer_names: &std::collections::HashMap<i64, String>,
 customers: &[abt_core::master_data::customer::model::Customer],
 params: &ReconciliationQueryParams,
 status_counts: &HashMap<i16, u64>,
 can_delete: bool,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();

 let total_count: u64 = status_counts.get(&0).copied().unwrap_or(0);
 let draft_count = status_counts.get(&1).copied();
 let sent_count = status_counts.get(&2).copied();
 let confirmed_count = status_counts.get(&3).copied();
 let disputed_count = status_counts.get(&4).copied();
 let settled_count = status_counts.get(&5).copied();

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: draft_count },
 TabItem { value: "2".into(), label: "已发送", count: sent_count },
 TabItem { value: "3".into(), label: "已确认", count: confirmed_count },
 TabItem { value: "4".into(), label: "有异议", count: disputed_count },
 TabItem { value: "5".into(), label: "已结算", count: settled_count },
 ];

 let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();
 let selected_period = params.period.as_deref().unwrap_or("");

 html! {
 div class="reconciliation-list-panel" {
 (status_tabs_with_param(ReconciliationListPath::PATH, "#reconciliation-data-card", "#reconciliation-filter-form", tabs, &active_value, "status"))

 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="reconciliation-filter-form"
 hx-get=(ReconciliationListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#reconciliation-data-card"
 hx-select="#reconciliation-data-card"
 hx-swap="outerHTML"
 hx-select-oob="#status-tabs"
 hx-include="#reconciliation-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
 placeholder="搜索对账单号、客户名称…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="customer_id" {
 option value="" { "全部客户" }
 @for c in customers {
 option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
 }
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="period" {
 option value="" selected[selected_period.is_empty()] { "对账期间" }
 @for p in generate_periods() {
 option value=(p.value) selected[selected_period == p.value] { (p.label) }
 }
 }
 }

 div class="data-card" id="reconciliation-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "对账单号" }
 th { "客户名称" }
 th { "对账期间" }
 th class="text-right text-[13px]" { "总金额" }
 th class="text-right text-[13px]" { "确认金额" }
 th class="text-right text-[13px]" { "差额" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for r in &result.items {
 (reconciliation_row(r, customer_names, can_delete))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="8" class="text-center p-8 text-muted" {
 "暂无对账数据"
 }
 }
 }
 }
 }
 }
 (pagination(ReconciliationListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
 }
}

struct PeriodOption {
 value: &'static str,
 label: &'static str,
}

fn generate_periods() -> Vec<PeriodOption> {
 let now = chrono::Local::now();
 let mut periods = vec![];
 for i in 0..6 {
 let d = now - chrono::Months::new(i);
 let value = d.format("%Y-%m").to_string();
 periods.push(PeriodOption {
 value: Box::leak(value.into_boxed_str()),
 label: Box::leak(d.format("%Y年%m月").to_string().into_boxed_str()),
 });
 }
 periods
}

fn reconciliation_row(
 r: &Reconciliation,
 customer_names: &std::collections::HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 let detail_path = ReconciliationDetailPath { id: r.id };
 let (status_text, status_class) = status_label(r.status);
 let customer_name = customer_names.get(&r.customer_id).map(|n| n.as_str()).unwrap_or("—");
 let onclick = format!("location.href='{}'", detail_path);
 let is_draft = r.status == ReconciliationStatus::Draft;
 let delete_path = ReconciliationDeletePath { id: r.id };

 html! {
 tr {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) { (r.doc_number) }
 td onclick=(&onclick) { (customer_name) }
 td onclick=(&onclick) { (r.period.as_str()) }
 td class="text-right text-[13px]" onclick=(&onclick) {
 span class="font-mono tabular-nums" { (crate::utils::fmt_amount(r.total_amount)) }
 }
 td class="text-right text-[13px]" onclick=(&onclick) {
 span class="font-mono tabular-nums" { (crate::utils::fmt_amount(r.confirmed_amount)) }
 }
 td class="text-right text-[13px]" onclick=(&onclick) {
 span class="font-mono tabular-nums font-semibold" { (crate::utils::fmt_amount(r.difference)) }
 }
 td onclick=(&onclick) {
 span class=(format!("status-pill {status_class}")) { (status_text) }
 }
 td onclick="event.stopPropagation()" {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 @if is_draft {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(detail_path.to_string()) title="编辑" {
 (icon::edit_icon("w-4 h-4"))
 }
 @if can_delete {
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
 hx-confirm=(format!("确认删除对账单 {}？", r.doc_number))
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
