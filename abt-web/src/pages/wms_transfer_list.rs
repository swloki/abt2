use axum::extract::Query;
use axum::response::Html;
use maud::{html, Markup};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use abt_core::wms::enums::TransferStatus;
use abt_core::wms::transfer::{InventoryTransfer, TransferService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_transfer::{TransferCreatePath, TransferDetailPath, TransferListPath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TransferQueryParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub doc_number: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_transfer_list(
 _path: TransferListPath,
 ctx: RequestContext,
 Query(params): Query<TransferQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let can_create = ctx.has_permission("INVENTORY", "create").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.transfer_service();

 let filter = build_filter(&params);
 let page = params.page.unwrap_or(1);
 let page_size = 20u32;

 let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

 let content = transfer_list_page(&result, &params, can_create);
 let page_html = admin_page(
 is_htmx,
 "库存调拨",
 &claims,
 "inventory",
 TransferListPath::PATH,
 "库存管理",
 Some("库存调拨"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &TransferQueryParams) -> abt_core::wms::transfer::TransferFilter {
 abt_core::wms::transfer::TransferFilter {
 doc_number: params.doc_number.clone(),
 status: params.status.and_then(TransferStatus::from_i16),
 from_warehouse_id: None,
 to_warehouse_id: None,
 }
}

// ── Components ──

fn transfer_list_page(
 result: &abt_core::shared::types::pagination::PaginatedResult<InventoryTransfer>,
 params: &TransferQueryParams,
 can_create: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "库存调拨" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(TransferCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建调拨"
 }
 }
 }
 }

 (transfer_table_fragment(result, params))
 }
 }
}

fn transfer_table_fragment(
 result: &abt_core::shared::types::pagination::PaginatedResult<InventoryTransfer>,
 params: &TransferQueryParams,
) -> Markup {
 let query = build_query_string(params);
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: None },
 TabItem { value: "2".into(), label: "在途", count: None },
 TabItem { value: "3".into(), label: "已完成", count: None },
 TabItem { value: "4".into(), label: "已取消", count: None },
 ];

 html! {
 div class="data-card" id="transfer-data-card" {
 (status_tabs(TransferListPath::PATH, "#transfer-data-card", ".filter-bar input, .filter-bar select", tabs, &active_value))

 form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="transfer-filter-form"
 hx-get=(TransferListPath::PATH)
 hx-trigger="change, keyup changed delay:300ms from:.search-input"
 hx-target="#transfer-data-card"
 hx-select="#transfer-data-card"
 hx-swap="outerHTML"
 hx-include="#transfer-filter-form"
 hx-push-url="true" {
 div class="relative flex-1 max-w-xs [&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 [&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="doc_number"
 placeholder="调拨单号";
 }
 }

 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "调拨单号" }
 th { "调出仓库" }
 th { "调入仓库" }
 th { "调拨日期" }
 th { "状态" }
 th class="text-right text-[13px]" { "物料项数" }
 th { "操作员" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for t in &result.items {
 (transfer_row(t))
 }
 @if result.items.is_empty() {
 tr {
 td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
 "暂无调拨数据"
 }
 }
 }
 }
 }
 }
 (pagination(TransferListPath::PATH, &query, result.total, result.page, result.total_pages))
 }
 }
}

fn transfer_row(t: &InventoryTransfer) -> Markup {
 let detail_path = TransferDetailPath { id: t.id };

 let (status_label, status_class) = match t.status {
 TransferStatus::Draft => ("草稿", "status-draft"),
 TransferStatus::InTransit => ("在途", "status-progress"),
 TransferStatus::Completed => ("已完成", "status-completed"),
 TransferStatus::Cancelled => ("已取消", "status-cancelled"),
 };

 html! {
 tr style="cursor:pointer" {
 td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (t.doc_number) }
 td onclick=(format!("location.href='{}'", detail_path)) { "—" }
 td onclick=(format!("location.href='{}'", detail_path)) { "—" }
 td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (t.transfer_date.to_string()) }
 td onclick=(format!("location.href='{}'", detail_path)) {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) { "—" }
 td onclick=(format!("location.href='{}'", detail_path)) { "—" }
 td onclick="event.stopPropagation()" {
 div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="查看" href=(detail_path.to_string()) {
 (icon::eye_icon("w-4 h-4"))
 }
 }
 }
 }
 }
}

fn build_query_string(params: &TransferQueryParams) -> String {
 let mut q = vec![];
 if let Some(s) = params.status {
 q.push(format!("status={s}"));
 }
 if let Some(ref d) = params.doc_number {
 q.push(format!("doc_number={d}"));
 }
 q.join("&")
}
