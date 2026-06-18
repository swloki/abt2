use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::strategy::model::*;
use abt_core::wms::strategy::StrategyService;
use abt_core::wms::enums::{PickType, PutawayType};

use crate::layout::page::admin_page;
use crate::routes::wms_strategy::StrategyListPath;
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_strategy_list(
 _path: StrategyListPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.strategy_service();

 let putaway_strategies = svc.list_putaway(&service_ctx, &mut conn, None).await?;
 let pick_strategies = svc.list_pick(&service_ctx, &mut conn, None).await?;

 let content = strategy_list_page(&putaway_strategies, &pick_strategies);
 let page_html = admin_page(
 is_htmx,
 "策略管理",
 &claims,
 "inventory",
 StrategyListPath::PATH,
 "库存管理",
 Some("策略管理"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn _putaway_type_label(t: &PutawayType) -> &'static str {
 match t {
 PutawayType::SameMerge => "同物料合并",
 PutawayType::Nearest => "就近入库",
 PutawayType::FixedBin => "指定库位",
 PutawayType::EmptyFirst => "空库位优先",
 }
}

fn putaway_type_tag(t: &PutawayType) -> &'static str {
 match t {
 PutawayType::SameMerge => "SAME_MERGE",
 PutawayType::Nearest => "NEAREST",
 PutawayType::FixedBin => "FIXED_BIN",
 PutawayType::EmptyFirst => "EMPTY_FIRST",
 }
}

fn _pick_type_label(t: &PickType) -> &'static str {
 match t {
 PickType::Fifo => "先进先出",
 PickType::Fefo => "先到期先出",
 PickType::ShortestPath => "最短路径",
 PickType::FullPallet => "整托优先",
 }
}

fn pick_type_tag(t: &PickType) -> &'static str {
 match t {
 PickType::Fifo => "FIFO",
 PickType::Fefo => "FEFO",
 PickType::ShortestPath => "SHORTEST_PATH",
 PickType::FullPallet => "FULL_PALLET",
 }
}

// ── Components ──

fn strategy_list_page(
 putaway_strategies: &[PutawayStrategy],
 pick_strategies: &[PickStrategy],
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "策略管理" }
 }

 // ── 上架策略 ──
 div class="mb-8" {
 div class="flex items-center justify-between mb-4" {
 div class="text-lg font-semibold text-fg flex items-center gap-2" { "上架策略" }
 }
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "策略名称" }
 th { "策略类型" }
 th { "适用仓库" }
 th { "产品分类" }
 th { "优先级" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for s in putaway_strategies {
 (putaway_row(s))
 }
 @if putaway_strategies.is_empty() {
 tr {
 td colspan="7" class="text-center text-muted py-8" {
 "暂无上架策略"
 }
 }
 }
 }
 }
 }
 }
 }

 // ── 拣货策略 ──
 div class="mb-8" {
 div class="flex items-center justify-between mb-4" {
 div class="text-lg font-semibold text-fg flex items-center gap-2" { "拣货策略" }
 }
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "策略名称" }
 th { "策略类型" }
 th { "适用仓库" }
 th { "产品分类" }
 th { "优先级" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for s in pick_strategies {
 (pick_row(s))
 }
 @if pick_strategies.is_empty() {
 tr {
 td colspan="7" class="text-center text-muted py-8" {
 "暂无拣货策略"
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
}

fn putaway_row(s: &PutawayStrategy) -> Markup {
 let tag = putaway_type_tag(&s.strategy_type);
 let status_text = if s.is_active { "启用" } else { "停用" };
 let toggle_active = if s.is_active { "active" } else { "" };

 html! {
 tr {
 td { strong class="font-medium" { (s.name) } }
 td {
 span class="inline-flex items-center rounded-full text-[11px] font-medium px-2.5 py-0.5 font-mono" style="background:#e8f4ff;color:#0958d9" { (tag) }
 }
 td {
 @if let Some(wid) = s.warehouse_id {
 "仓库#" (wid)
 } @else {
 span class="text-muted" { "全部仓库" }
 }
 }
 td {
 @if let Some(cid) = s.product_category_id {
 "分类#" (cid)
 } @else {
 span class="text-muted" { "全部" }
 }
 }
 td { (priority_badge(s.priority)) }
 td {
 label class="flex items-center gap-2 text-sm text-fg-2 cursor-pointer whitespace-nowrap"
 _="on click toggle .active on closest .toggle-track" {
 span class=(format!("toggle-track w-9 h-5 rounded-full relative shrink-0 transition-colors duration-150 bg-border act:bg-success after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:w-4 after:h-4 after:bg-white after:rounded-full after:transition-transform after:duration-150 after:shadow-[0_1px_3px_rgba(0,0,0,0.15)] act:after:translate-x-4 {}", toggle_active)) {}
 (status_text)
 }
 }
 td {
 div class="flex items-center gap-1 justify-end" {
 button class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer hover:bg-accent-bg" title="编辑" {
 (crate::components::icon::edit_icon("w-4 h-4"))
 }
 }
 }
 }
 }
}

fn pick_row(s: &PickStrategy) -> Markup {
 let tag = pick_type_tag(&s.strategy_type);
 let status_text = if s.is_active { "启用" } else { "停用" };
 let toggle_active = if s.is_active { "active" } else { "" };

 html! {
 tr {
 td { strong class="font-medium" { (s.name) } }
 td {
 span class="inline-flex items-center rounded-full text-[11px] font-medium px-2.5 py-0.5 font-mono" class="text-[#389e0d]" style="background:#f0fff0" { (tag) }
 }
 td {
 @if let Some(wid) = s.warehouse_id {
 "仓库#" (wid)
 } @else {
 span class="text-muted" { "全部仓库" }
 }
 }
 td { span class="text-muted" { "全部" } }
 td { (priority_badge(s.priority)) }
 td {
 label class="flex items-center gap-2 text-sm text-fg-2 cursor-pointer whitespace-nowrap"
 _="on click toggle .active on closest .toggle-track" {
 span class=(format!("toggle-track w-9 h-5 rounded-full relative shrink-0 transition-colors duration-150 bg-border act:bg-success after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:w-4 after:h-4 after:bg-white after:rounded-full after:transition-transform after:duration-150 after:shadow-[0_1px_3px_rgba(0,0,0,0.15)] act:after:translate-x-4 {}", toggle_active)) {}
 (status_text)
 }
 }
 td {
 div class="flex items-center gap-1 justify-end" {
 button class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer hover:bg-accent-bg" title="编辑" {
 (crate::components::icon::edit_icon("w-4 h-4"))
 }
 }
 }
 }
 }
}

fn priority_badge(p: i32) -> Markup {
 let p = p.clamp(1, 4);
 let (bg, color) = match p {
 1 => ("#fff2f0", "#cf1322"),
 2 => ("#fff8eb", "#d46b08"),
 3 => ("#e8f4ff", "#0958d9"),
 _ => ("var(--surface)", "var(--muted)"),
 };
 html! {
 span class="inline-flex items-center justify-center w-7 h-7 rounded-sm text-sm font-bold font-mono" style=(format!("background:{};color:{}", bg, color)) { (p) }
 }
}
