use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::*;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::supplier::{
 SupplierCreatePath, SupplierDeletePath, SupplierDetailPath, SupplierListPath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SupplierQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub category: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("SUPPLIER", "read")]
pub async fn get_supplier_list(
 _path: SupplierListPath,
 ctx: RequestContext,
 Query(params): Query<SupplierQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("SUPPLIER", "create").await;
 let can_delete = ctx.has_permission("SUPPLIER", "delete").await;
 let can_edit = ctx.has_permission("SUPPLIER", "update").await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.supplier_service();

 let filter = build_filter(&params);
 let page = PageParams::new(params.page.unwrap_or(1), 20);

 let result = svc.list(&service_ctx, &mut conn, filter, page).await?;


 let content = supplier_list_page(&result, &params, can_create, can_delete, can_edit);
 let page_html = admin_page(
 is_htmx,
 "供应商管理",
 &claims,
 "purchase",
 SupplierListPath::PATH,
 "主数据管理",
 Some("供应商管理"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SUPPLIER", "delete")]
pub async fn delete_supplier(
 path: SupplierDeletePath,
 ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.supplier_service();

 svc.delete(&service_ctx, &mut conn, path.id).await?;

 Ok(([("HX-Redirect", SupplierListPath::PATH)], Html(String::new())))
}

// ── Helpers ──

fn build_filter(params: &SupplierQueryParams) -> SupplierQuery {
 SupplierQuery {
 name: params.keyword.clone(),
 status: params.status.and_then(SupplierStatus::from_i16),
 category: params.category.and_then(SupplierCategory::from_i16),
 }
}

// ── Components ──

fn supplier_list_page(
 result: &abt_core::shared::types::PaginatedResult<Supplier>,
 params: &SupplierQueryParams,
 can_create: bool,
 can_delete: bool,
 can_edit: bool,
) -> Markup {
 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "供应商管理" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(SupplierCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建供应商"
 }
 }
 }
 }


 // ── Tabs + Filter + Data Table (HTMX panel) ──
 (supplier_table_fragment(result, params, can_delete, can_edit))
 }
 }
}

fn supplier_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<Supplier>,
 params: &SupplierQueryParams,
 can_delete: bool,
 can_edit: bool,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "2".into(), label: "合格", count: None },
 TabItem { value: "3".into(), label: "试用期", count: None },
 TabItem { value: "1".into(), label: "潜在", count: None },
 TabItem { value: "4".into(), label: "不合格", count: None },
 TabItem { value: "5".into(), label: "黑名单", count: None },
 ];

 html! {
 div class="supplier-list-panel" {
 (status_tabs_with_param(SupplierListPath::PATH, "#supplier-data-card", "#supplier-filter-form", tabs, &active_value, "status"))

 // ── Filter Bar ──
 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="supplier-filter-form"
 hx-get=(SupplierListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#supplier-data-card"
 hx-select="#supplier-data-card"
 hx-swap="outerHTML"
 hx-select-oob="#status-tabs"
 hx-include="#supplier-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="keyword"
 placeholder="搜索供应商名称、编码…"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="category" {
 option value="" { "全部类别" }
 option value="1" selected[params.category == Some(1)] { "原材料" }
 option value="2" selected[params.category == Some(2)] { "包装材料" }
 option value="3" selected[params.category == Some(3)] { "外协加工" }
 option value="4" selected[params.category == Some(4)] { "辅料" }
 option value="5" selected[params.category == Some(5)] { "服务" }
 }
 }

 // ── Data Table ──
 div class="data-card" id="supplier-data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "供应商编码" }
 th { "供应商名称" }
 th { "供应类别" }
 th { "联系人" }
 th { "电话" }
 th { "交货天数" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for s in &result.items {
 (supplier_row(s, can_delete, can_edit))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="8" class="text-center text-muted py-8" {
 "暂无供应商数据"
 }
 }
 }
 }
 }
 }
 (pagination(SupplierListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
 }
}

fn supplier_row(s: &Supplier, can_delete: bool, can_edit: bool) -> Markup {
 let detail_path = SupplierDetailPath { id: s.id };
 let delete_path = SupplierDeletePath { id: s.id };

 let category_label = match s.category {
 SupplierCategory::RawMaterial => "原材料",
 SupplierCategory::Packaging => "包装材料",
 SupplierCategory::Outsourcing => "外协加工",
 SupplierCategory::Consumable => "辅料",
 SupplierCategory::Service => "服务",
 };

 let (status_label, status_class) = match s.status {
 SupplierStatus::Prospective => ("潜在", "status-draft"),
 SupplierStatus::Qualified => ("合格", "status-accepted"),
 SupplierStatus::Probation => ("试用期", "status-progress"),
 SupplierStatus::Disqualified => ("不合格", "status-rejected"),
 SupplierStatus::Blacklisted => ("黑名单", "status-rejected"),
 };

 html! {
 tr class="cursor-pointer" {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (s.code) }
 td onclick=(format!("location.href='{}'", detail_path)) { strong { (s.name) } }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-surface text-slate-500" { (category_label) }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class="text-muted" { "—" }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class="text-muted" { "—" }
 }
 td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) {
 @if s.lead_time_days > 0 {
 (s.lead_time_days) " 天"
 } @else {
 span class="text-muted" { "—" }
 }
 }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 td _="on click halt the event" {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg icon:w-3.5 icon:h-3.5" {
 @if can_edit {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑"
 href=(SupplierDetailPath { id: s.id }.to_string()) {
 (icon::edit_icon("w-4 h-4"))
 }
 }
 @if can_delete {
 button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
 hx-post=(delete_path)
 hx-confirm=(format!("删除后无法恢复，确定要删除供应商 <strong>{}</strong> 吗？", s.name))
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

fn build_query_string(params: &SupplierQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref kw) = params.keyword {
 q.push(format!("keyword={kw}"));
 }
 if let Some(s) = params.status {
 q.push(format!("status={s}"));
 }
 if let Some(c) = params.category {
 q.push(format!("category={c}"));
 }
 q.join("&")
}
