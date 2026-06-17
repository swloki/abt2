use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_plan::ProductionPlanService;

use abt_core::mes::enums::PlanStatus;
use abt_core::mes::production_plan::{PlanExtraStats, PlanFilter, ProductionPlan};
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_plan::{PlanCreatePath, PlanDetailPath, PlanListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PlanQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub plan_type: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_from: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_to: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn plan_status_label(s: &PlanStatus) -> (&'static str, &'static str, &'static str) {
 match s {
 PlanStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
 PlanStatus::Confirmed => ("已确认", "rgba(22,119,255,0.08)", "var(--accent)"),
 PlanStatus::InProgress => ("进行中", "rgba(250,140,22,0.08)", "#fa8c16"),
 PlanStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
 PlanStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
 }
}

fn plan_type_label(t: &str) -> (&'static str, &'static str, &'static str) {
 match t {
 "Mto" => ("MTO", "rgba(22,119,255,0.08)", "var(--accent)"),
 "Mts" => ("MTS", "rgba(114,46,209,0.08)", "#722ed1"),
 _ => ("—", "rgba(0,0,0,0.04)", "var(--muted)"),
 }
}

fn parse_status(s: &str) -> Option<PlanStatus> {
 match s {
 "Draft" => Some(PlanStatus::Draft),
 "Confirmed" => Some(PlanStatus::Confirmed),
 "InProgress" => Some(PlanStatus::InProgress),
 "Completed" => Some(PlanStatus::Completed),
 "Cancelled" => Some(PlanStatus::Cancelled),
 _ => None,
 }
}

async fn resolve_operator_names<S: UserService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[ProductionPlan],
) -> HashMap<i64, String> {
 let mut map = HashMap::new();
 for item in items {
 if !map.contains_key(&item.operator_id)
 && let Ok(user) = svc.get_user(ctx, db, item.operator_id).await {
 map.insert(item.operator_id, user.display_name.unwrap_or_default());
 }
 }
 map
}

fn build_query_string(params: &PlanQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref v) = params.keyword {
 q.push(format!("keyword={v}"));
 }
 if let Some(ref v) = params.status {
 q.push(format!("status={v}"));
 }
 if let Some(ref v) = params.plan_type {
 q.push(format!("plan_type={v}"));
 }
 if let Some(ref v) = params.date_from {
 q.push(format!("date_from={v}"));
 }
 if let Some(ref v) = params.date_to {
 q.push(format!("date_to={v}"));
 }
 q.join("&")
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_plan_list(
 _path: PlanListPath,
 ctx: RequestContext,
 Query(params): Query<PlanQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("WORK_ORDER", "create").await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.production_plan_service();
 let user_svc = state.user_service();

 let filter = PlanFilter {
 status: params.status.as_deref().and_then(parse_status),
 plan_type: params.plan_type.as_deref().and_then(|t| match t {
 "Mto" => Some(abt_core::mes::enums::PlanType::Mto),
 "Mts" => Some(abt_core::mes::enums::PlanType::Mts),
 _ => None,
 }),
 keyword: params.keyword.clone(),
 date_from: params.date_from.as_deref().and_then(|d| d.parse().ok()),
 date_to: params.date_to.as_deref().and_then(|d| d.parse().ok()),
 };
 let page_num = params.page.unwrap_or(1);
 let result = svc.list(&service_ctx, &mut conn, filter, page_num, 20).await?;
 let operator_names = resolve_operator_names(&user_svc, &service_ctx, &mut conn, &result.items).await;
 let plan_ids: Vec<i64> = result.items.iter().map(|p| p.id).collect();
 let plan_stats = svc.get_plan_stats(&service_ctx, &mut conn, &plan_ids).await?;
 let content = plan_list_page(&result, &operator_names, &plan_stats, &params, can_create);
 let page_html = admin_page(
 is_htmx, "生产计划", &claims, "production", PlanListPath::PATH, "生产管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn plan_list_page(
 result: &abt_core::shared::types::PaginatedResult<ProductionPlan>,
 operator_names: &HashMap<i64, String>,
 plan_stats: &HashMap<i64, PlanExtraStats>,
 params: &PlanQueryParams,
 can_create: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "生产计划" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(PlanCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建计划"
 }
 }
 }
 }
 (plan_table_fragment(result, operator_names, plan_stats, params))
 }
 }
}
fn plan_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<ProductionPlan>,
 operator_names: &HashMap<i64, String>,
 plan_stats: &HashMap<i64, PlanExtraStats>,
 params: &PlanQueryParams,
) -> Markup {
 let total_count = result.total;
 let selected_status = params.status.as_deref().unwrap_or("");

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "Draft".into(), label: "草稿", count: None },
 TabItem { value: "Confirmed".into(), label: "已确认", count: None },
 TabItem { value: "InProgress".into(), label: "进行中", count: None },
 TabItem { value: "Completed".into(), label: "已完成", count: None },
 TabItem { value: "Cancelled".into(), label: "已取消", count: None },
 ];

 html! {
 div class="plan-list-panel" {
 (status_tabs_with_param(PlanListPath::PATH, "#plan-data-card", "#filter-form", tabs, selected_status, "status"))

 // ── Filter Bar ──
 form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
 hx-get=(PlanListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#plan-data-card"
 hx-select="#plan-data-card"
 hx-swap="outerHTML"
 hx-include="#filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs" {
 (icon::search_icon("w-4 h-4"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
 style="width:180px"
 placeholder="搜索计划编号…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="plan_type" {
 option value="" selected[params.plan_type.is_none()] { "全部类型" }
 option value="Mto" selected[params.plan_type.as_deref() == Some("Mto")] { "按单生产 (MTO)" }
 option value="Mts" selected[params.plan_type.as_deref() == Some("Mts")] { "按库存备货 (MTS)" }
 }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_from"
 style="max-width:160px"
 value=(params.date_from.as_deref().unwrap_or(""));
 span style="color:var(--muted);font-size:13px" { "至" }
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_to"
 style="max-width:160px"
 value=(params.date_to.as_deref().unwrap_or(""));
 }

 // ── Data Table ──
 (plan_data_card(result, operator_names, plan_stats, params))
 }
 }
}
fn plan_data_card(
 result: &abt_core::shared::types::PaginatedResult<ProductionPlan>,
 operator_names: &HashMap<i64, String>,
 plan_stats: &HashMap<i64, PlanExtraStats>,
 params: &PlanQueryParams,
) -> Markup {
 let query = build_query_string(params);
 html! {
 div class="data-card" id="plan-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "计划编号" }
 th { "计划日期" }
 th { "排产类型" }
 th { "计划行数" }
 th { "关联销售单" }
 th { "状态" }
 th { "创建人" }
 th { "创建时间" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for item in &result.items {
 @let (status_label, status_bg, status_color) = plan_status_label(&item.status);
 @let (type_label, type_bg, type_color) = plan_type_label(&item.plan_type.to_string());
 @let op_name = operator_names.get(&item.operator_id).map(|s| s.as_str()).unwrap_or("—");
 @let detail_path = PlanDetailPath { id: item.id };
 @let stats = plan_stats.get(&item.id);
 @let item_count = stats.map(|s| s.item_count).unwrap_or(0);
 @let sales_orders = stats.map(|s| s.sales_orders.as_str()).unwrap_or("—");
 @let sales_orders_display = if sales_orders.is_empty() { "—" } else { sales_orders };
 tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="color:var(--accent)" { (item.doc_number) }
 td { (item.plan_date) }
 td {
 span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
 (type_label)
 }
 }
 td style="text-align:center" { (item_count) }
 td { (sales_orders_display) }
 td {
 span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
 (status_label)
 }
 }
 td { (op_name) }
 td style="font-size:12px;color:var(--muted)" { (item.created_at.format("%Y-%m-%d %H:%M")) }
 td {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 a href=(detail_path.to_string()) style="color:var(--accent);font-size:var(--text-xs)" { "查看" }
 }
 }
 }
 }
 @if result.items.is_empty() {
 tr {
 td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
 "暂无生产计划"
 }
 }
 }
 }
 }
 }
 (pagination(PlanListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
}
