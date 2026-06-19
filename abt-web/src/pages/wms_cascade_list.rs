use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::inventory_cascade::model::*;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::wms_cascade::CascadeListPath;
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_cascade_list(
 _path: CascadeListPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let claims = ctx.claims;

 let content = cascade_page(None);
 let page_html = admin_page(
 is_htmx,
 "级联库存查询",
 &claims,
 "inventory",
 CascadeListPath::PATH,
 "库存管理",
 Some("级联库存查询"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn cascade_page(result: Option<&CascadeInventoryResult>) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "级联库存查询" }
 }

 // ── Search bar ──
 div class="flex items-center gap-3 bg-bg border border-border-soft rounded-md py-5 px-6 mb-6" {
 div class="relative flex-1 icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted" {
 (icon::search_icon(""))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="product_code"
 placeholder="输入产品编码或产品名称"
 hx-get=(CascadeListPath::PATH)
 hx-trigger="keyup changed delay:500ms"
 hx-sync="this:replace"
 hx-target=".cascade-results"
 hx-swap="innerHTML";
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-get=(CascadeListPath::PATH)
 hx-target=".cascade-results"
 hx-swap="innerHTML"
 hx-include="input[name=product_code]" {
 (icon::search_icon("w-4 h-4"))
 "查询"
 }
 }

 // ── Results ──
 div class="cascade-results" {
 @if let Some(r) = result {
 (cascade_results(r))
 } @else {
 div class="text-center py-8 text-muted" {
 "请输入产品编码进行查询"
 }
 }
 }
 }
 }
}

fn cascade_results(result: &CascadeInventoryResult) -> Markup {
 html! {
 div {
 div class="flex items-center gap-5 p-5 px-6 mb-6 rounded-md bg-[linear-gradient(135deg,#e6f4ff_0%,#f0f7ff_100%)] border border-[rgba(22,119,255,0.15)]" {
 div class="w-12 h-12 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,var(--accent)_0%,#4096ff_100%)] text-white" {
 (icon::box_icon("w-6 h-6"))
 }
 div class="flex-1" {
 div class="text-lg font-bold text-fg mb-1" { (result.product_name) }
 div class="text-sm text-muted font-mono" { (result.product_code) }
 }
 div class="text-right" {
 div class="text-xs text-muted" { "当前库存总量" }
 div class="text-2xl font-bold font-mono text-fg" {
 (format!("{:.2}", result.total_quantity))
 }
 }
 }
 @for group in &result.bom_groups {
 (bom_group(group))
 }
 @if result.bom_groups.is_empty() {
 div class="text-center py-8 text-muted" {
 "该产品无关联BOM"
 }
 }
 }
 }
}

fn bom_group(group: &BomCascadeGroup) -> Markup {
 html! {
 div class="mb-6" {
 div class="flex items-center gap-3 mb-3" {
 span class="inline-flex items-center gap-1 px-3 py-0.5 rounded-full text-xs font-semibold bg-accent-bg text-accent" {
 (icon::box_icon("w-3.5 h-3.5"))
 "BOM"
 }
 span class="text-base font-semibold text-fg" {
 (group.bom_name)
 }
 }
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "子件编码" }
 th { "子件名称" }
 th { "单位" }
 th class="text-right text-[13px]" { "BOM用量" }
 th class="text-right text-[13px]" { "当前库存总量" }
 th class="text-right text-[13px]" { "损耗率" }
 th { "是否缺料" }
 }
 }
 tbody {
 @for child in &group.children {
 (bom_child_row(child))
 }
 @if group.children.is_empty() {
 tr {
 td colspan="7" class="text-center py-8 text-muted" {
 "无子件数据"
 }
 }
 }
 }
 }
 }
 }
 }
 }
}
fn bom_child_row(child: &ChildNodeInventory) -> Markup {
 let is_shortage = child.total_stock < child.quantity;
 let loss_pct = child.loss_rate * Decimal::from(100);

 html! {
 tr {
 td class="font-mono tabular-nums" { (child.product_code) }
 td { (child.product_name) }
 td {
 @if let Some(ref u) = child.unit {
 (u)
 } @else {
 span class="text-muted" { "—" }
 }
 }
 td class="text-right text-[13px] font-mono tabular-nums" { (child.quantity) }
 td class="text-right text-[13px] font-mono tabular-nums" { (child.total_stock) }
 td class="text-right text-[13px] font-mono tabular-nums" {
 (format!("{:.1}%", loss_pct))
 }
 td {
 @if is_shortage {
 span class="inline-flex items-center gap-1 text-danger font-semibold font-mono" {
 (crate::components::icon::circle_alert_icon("w-3.5 h-3.5"))
 "缺料"
 }
 } @else {
 span class="text-success font-medium font-mono" {
 "充足"
 }
 }
 }
 }
}
}
