use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::work_center::{model::*, WorkCenterService};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::md_work_center::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WorkCenterQueryParams {
 #[serde(default)]
 pub keyword: Option<String>,
 #[serde(default)]
 pub is_active: Option<String>,
 #[serde(default)]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("BOM", "read")]
pub async fn get_work_center_list(
 _path: WorkCenterListPath,
 ctx: RequestContext,
 Query(params): Query<WorkCenterQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let page = params.page.unwrap_or(1);
 let keyword = params
 .keyword
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|s| s.to_string());
 let is_active = match params.is_active.as_deref() {
 Some("true") => Some(true),
 Some("false") => Some(false),
 _ => None,
 };

 let filter = WorkCenterFilter {
 keyword,
 work_center_type: None,
 is_active,
 };
 let result = state
 .work_center_service()
 .list(&service_ctx, &mut conn, filter, PageParams::new(page, 20))
 .await?;

 let content = work_center_list_page(&result, &params);
 Ok(Html(
 admin_page(
 is_htmx,
 "工作中心管理",
 &claims,
 "md",
 WorkCenterListPath::PATH,
 "工程",
        Some("工作中心管理"),
 content,
 &nav_filter,
 )
 .into_string(),
 ))
}

// ── Components ──

fn work_center_list_page(
 result: &abt_core::shared::types::PaginatedResult<WorkCenter>,
 params: &WorkCenterQueryParams,
) -> Markup {
 let total = result.total;
 let page = params.page.unwrap_or(1);
 let page_size = 20u32;
 let total_pages = (total as u32).div_ceil(page_size);
 let query_string = build_query_string(params);

 html! {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-2xl font-bold text-fg tracking-tight" { "工作中心管理" }
 div class="flex gap-3" {
a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(WorkCenterCreatePath::PATH) {
(icon::plus_icon("w-4 h-4"))
"新建工作中心"
}
}
}

 // 筛选栏
 div class="flex items-center gap-3 mb-5 flex-wrap" {
 form class="filter-form flex items-center gap-3" id="wc-filter-form"
 hx-get=(WorkCenterListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#data-card"
 hx-select="#data-card"
 hx-swap="outerHTML"
 hx-push-url="true"
 hx-include="#wc-filter-form" {

 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="is_active" {
 @if params.is_active.is_none() {
 option value="" selected { "全部" }
 } @else {
 option value="" { "全部" }
 }
 @if params.is_active.as_deref() == Some("true") {
 option value="true" selected { "启用" }
 } @else {
 option value="true" { "启用" }
 }
 @if params.is_active.as_deref() == Some("false") {
 option value="false" selected { "停用" }
 } @else {
 option value="false" { "停用" }
 }
 }

 div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted" {
 (icon::search_icon("w-4 h-4"))
 input class="search-input w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="keyword"
 placeholder="搜索编码 / 名称"
 value=(params.keyword.as_deref().unwrap_or(""));
 }
 }
 }

 // 数据表
 div class="data-card" id="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "编码" }
 th { "名称" }
 th { "类型" }
 th class="text-right text-[13px]" { "产能/小时" }
 th class="text-right text-[13px]" { "成本费率/h" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for wc in &result.items {
 tr {
 td class="font-mono tabular-nums" { (wc.code) }
 td { strong { (wc.name) } }
 td { (work_center_type_label(wc.work_center_type)) }
 td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(wc.default_capacity)) }
 td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_amount(wc.costs_hour)) }
 td {
 @if wc.is_active {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-success-bg text-success" { "启用" }
 } @else {
 span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-danger-bg text-danger" { "停用" }
 }
 }
 td class="text-center whitespace-nowrap" {
a class="inline-flex items-center justify-center w-[28px] h-[28px] border-none bg-surface rounded-sm cursor-pointer hover:bg-accent-bg hover:text-accent transition-colors no-underline text-fg-2" href=(WorkCenterDetailPath { id: wc.id }.to_string()) title="查看" {
(icon::eye_icon("w-3.5 h-3.5"))
}
a class="inline-flex items-center justify-center w-[28px] h-[28px] border-none bg-surface rounded-sm cursor-pointer hover:bg-accent-bg hover:text-accent transition-colors no-underline ml-1 text-fg-2" href=(WorkCenterEditPath { id: wc.id }.to_string()) title="编辑" {
(icon::edit_icon("w-3.5 h-3.5"))
}
}
 }
 }
 @if result.items.is_empty() {
 tr { td colspan="7" class="text-center text-muted text-sm" { "暂无工作中心数据" } }
 }
 }
 }
 }
 (pagination(WorkCenterListPath::PATH, &query_string, total, page, total_pages))
 }
 }
}

// ── Helpers ──

fn build_query_string(params: &WorkCenterQueryParams) -> String {
 let mut parts = Vec::new();
 if let Some(ref k) = params.keyword
 && !k.is_empty()
 {
 parts.push(format!("keyword={}", k));
 }
 if let Some(ref a) = params.is_active
 && !a.is_empty()
 {
 parts.push(format!("is_active={}", a));
 }
 parts.join("&")
}
