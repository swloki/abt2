use std::collections::HashMap;

use axum::response::Html;
use maud::{html, Markup};
use rust_decimal::Decimal;

use crate::errors::Result;
use crate::routes::wms_backflush::BackflushDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use abt_core::wms::backflush::{BackflushItem, BackflushService};
use abt_core::wms::enums::BackflushStatus;
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;
use crate::components::icon;

// ── Resolved Component Info ──

struct ComponentInfo {
 codes: HashMap<i64, String>,
 names: HashMap<i64, String>,
 units: HashMap<i64, String>,
}

impl ComponentInfo {
 fn code(&self, id: &i64) -> &str { self.codes.get(id).map(|s| s.as_str()).unwrap_or("—") }
 fn name(&self, id: &i64) -> &str { self.names.get(id).map(|s| s.as_str()).unwrap_or("—") }
 fn unit(&self, id: &i64) -> &str { self.units.get(id).map(|s| s.as_str()).unwrap_or("—") }
}

#[require_permission("INVENTORY", "read")]
pub async fn get_backflush_detail(
 path: BackflushDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.backflush_service();

 let record = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = svc.get_items(&service_ctx, &mut conn, path.id).await?;

 // Resolve operator name
 let operator_name = state.user_service()
 .get_user(&service_ctx, &mut conn, record.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // Resolve finished product name
 let product_name = state.product_service()
 .get(&service_ctx, &mut conn, record.product_id)
 .await
 .map(|p| format!("{} ({})", p.pdt_name, p.product_code))
 .unwrap_or_else(|_| "—".into());

 // Resolve component product names for items
 let product_svc = state.product_service();
 let mut item_product_codes: HashMap<i64, String> = HashMap::new();
 let mut item_product_names: HashMap<i64, String> = HashMap::new();
 let mut item_product_units: HashMap<i64, String> = HashMap::new();
 for item in &items {
 if item_product_codes.contains_key(&item.component_id) {
 continue;
 }
 if let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.component_id).await {
 item_product_codes.insert(item.component_id, p.product_code.clone());
 item_product_names.insert(item.component_id, p.pdt_name.clone());
 item_product_units.insert(item.component_id, p.unit.clone());
 }
 }

 let component_info = ComponentInfo { codes: item_product_codes, names: item_product_names, units: item_product_units };
 let content = backflush_detail_page(
 &record, &items,
 &operator_name, &product_name, &component_info,
 );
 let page_html = admin_page(
 is_htmx,
 "倒冲记录详情",
 &claims,
 "inventory",
 "/admin/wms/backflushes",
 "库存管理",
 None,
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

fn backflush_detail_page(
 record: &abt_core::wms::backflush::BackflushRecord,
 items: &[BackflushItem],
 operator_name: &str,
 product_name: &str,
 component_info: &ComponentInfo,
) -> Markup {
 let (status_label, status_class) = match record.status {
 BackflushStatus::Draft => ("草稿", "status-draft"),
 BackflushStatus::Executed => ("已执行", "status-completed"),
 BackflushStatus::Adjusted => ("已调整", "status-confirmed"),
 };

 let over_count = items.iter().filter(|i| i.is_over_threshold).count();
 let max_rate = items.iter()
 .map(|i| i.variance_rate.abs())
 .max()
 .unwrap_or(Decimal::ZERO);

 let show_adjust = matches!(record.status, BackflushStatus::Executed);

 html! {
 div {
 // ── Back Link ──
 a href="/admin/wms/backflushes" class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回倒冲记录列表"
 }
 // ── Detail Header（裸 flex，非 card）──
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center gap-4" {
 h1 class="text-xl font-bold font-mono tabular-nums" { (record.doc_number) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::printer_icon("w-4 h-4"))
 "打印"
 }
 @if show_adjust {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "确认调整"
 }
 }
 }
 }

 // ── Status Flow ──
 (backflush_status_flow(record.status))

 // ── Info Card ──
 div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "倒冲信息" }
 div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))]" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "单据编号" }
 span class="text-sm text-fg font-mono tabular-nums" { (record.doc_number) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "关联工单" }
 span class="text-sm text-fg font-mono tabular-nums" { "—" }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "完工产品" }
 span class="text-sm text-fg" { (product_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "完工数量" }
 span class="text-sm text-fg font-mono tabular-nums" { (format!("{:.2}", record.completed_qty)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "倒冲日期" }
 span class="text-sm text-fg font-mono tabular-nums" { (record.backflush_date.to_string()) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "差异阈值" }
 span class="text-sm text-fg font-mono tabular-nums" { (format!("{:.2}%", record.variance_threshold)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "状态" }
 span class="text-sm text-fg" {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "操作员" }
 span class="text-sm text-fg" { (operator_name) }
 }
 }
 }

 // ── Items Table ──
 div class="data-card" {
 div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "倒冲物料明细" }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "行号" }
 th { "子件编码" }
 th { "子件名称" }
 th { "单位" }
 th class="text-right text-[13px]" { "BOM理论用量" }
 th class="text-right text-[13px]" { "实际倒冲量" }
 th class="text-right text-[13px]" { "差异量" }
 th class="text-right text-[13px]" { "差异率" }
 th class="text-right text-[13px]" { "超标" }
 }
 }
 tbody {
 @for (i, item) in items.iter().enumerate() {
 (backflush_item_row(i + 1, item, component_info))
 }
 @if items.is_empty() {
 tr {
 td colspan="9" class="text-center text-muted py-8" {
 "暂无明细数据"
 }
 }
 }
 }
 }
 }
 }

 // ── Summary Bar ──
 @if !items.is_empty() {
 div class="grid grid-cols-3 gap-4 mt-5" {
 div class="text-center p-4 bg-surface border border-border-soft rounded-md" {
 div class="text-xl font-bold text-fg font-mono tabular-nums" { (items.len()) }
 div class="text-xs text-muted mt-1" { "总子件数" }
 }
 div class="text-center p-4 bg-surface border border-border-soft rounded-md" {
 div class=(format!("text-xl font-bold font-mono tabular-nums {}", if over_count > 0 { "text-danger" } else { "text-fg" })) { (over_count) }
 div class="text-xs text-muted mt-1" { "超标项数" }
 }
 div class="text-center p-4 bg-surface border border-border-soft rounded-md" {
 div class=(format!("text-xl font-bold font-mono tabular-nums {}", if max_rate > Decimal::ZERO { "text-danger" } else { "text-fg" })) {
 @if max_rate > Decimal::ZERO {
 "+" (format!("{:.2}", max_rate)) "%"
 } @else {
 "0%"
 }
 }
 div class="text-xs text-muted mt-1" { "最大差异率" }
 }
 }
 }
 }
 }
}

fn backflush_item_row(
 idx: usize,
 item: &BackflushItem,
 component_info: &ComponentInfo,
) -> Markup {
 let variance_sign = if item.variance_qty >= Decimal::ZERO { "+" } else { "" };
 let rate_sign = if item.variance_rate >= Decimal::ZERO { "+" } else { "" };
 let has_variance = item.variance_qty != Decimal::ZERO;
 let variance_cls = if has_variance { " text-danger" } else { "" };

 html! {
 tr {
 td class="font-mono tabular-nums" { (idx) }
 td class="font-mono tabular-nums" { (component_info.code(&item.component_id)) }
 td { (component_info.name(&item.component_id)) }
 td { (component_info.unit(&item.component_id)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.theoretical_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.actual_qty)) }
 td class=(format!("text-right text-[13px] font-mono tabular-nums{variance_cls}")) {
 (variance_sign) (format!("{:.2}", item.variance_qty))
 }
 td class=(format!("text-right text-[13px] font-mono tabular-nums{variance_cls}")) {
 (rate_sign) (format!("{:.2}", item.variance_rate)) "%"
 }
 td class="text-right text-[13px]" {
 @if item.is_over_threshold {
 span class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-danger-bg text-danger text-xs font-bold" { "✓" }
 } @else {
 span class="text-muted" { "✗" }
 }
 }
 }
 }
}

fn backflush_status_flow(status: BackflushStatus) -> Markup {
 let steps = [
 ("草稿", BackflushStatus::Draft),
 ("已执行", BackflushStatus::Executed),
 ("已调整", BackflushStatus::Adjusted),
 ];

 let current_idx = match status {
 BackflushStatus::Draft => 0,
 BackflushStatus::Executed => 1,
 BackflushStatus::Adjusted => 2,
 };

 html! {
 div class="flex items-center gap-2 mt-6 mb-6" {
 @for (i, (label, _)) in steps.iter().enumerate() {
 @if i > 0 {
 span class="text-border text-sm" { "→" }
 }
 @let dot_cls = if i < current_idx { "bg-success" }
 else if i == current_idx { "bg-accent ring-[3px] ring-[rgba(37,99,235,0.1)]" }
 else { "bg-[#d1d5db]" };
 @let text_cls = if i <= current_idx { "text-fg" } else { "text-muted" };
 div class="flex items-center gap-2 shrink-0" {
 span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {}", dot_cls)) {}
 span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
 }
 }
 }
 }
}
