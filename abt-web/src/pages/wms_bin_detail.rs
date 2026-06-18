use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::{Result, error_page};
use crate::layout::page::admin_page;
use crate::pages::wms_bin_list::{bin_status_class, bin_status_label};
use crate::routes::wms_bin::{BinDetailPath, BinListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("LOCATION", "read")]
pub async fn get_bin_detail(
 path: BinDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.warehouse_service();

 let bww = match svc.get_bin_with_warehouse(&service_ctx, &mut conn, path.id).await {
 Ok(bww) => bww,
 Err(e) => {
 if matches!(e, abt_core::shared::types::DomainError::NotFound(_)) {
 let content = error_page("库位未找到", &format!("库位 ID {} 不存在或已被删除", path.id));
 let page_html = admin_page(
 is_htmx,
 "库位未找到",
 &claims,
 "inventory",
 &BinListPath.to_string(),
 "库存管理",
 Some("库位未找到"),
 content, &nav_filter, );
 return Ok(Html(page_html.into_string()));
 }
 return Err(e.into());
 }
 };
 let zones = svc.list_zones(&service_ctx, &mut conn, bww.warehouse_id).await?;
 let zone = zones.iter().find(|z| z.id == bww.bin.zone_id);
 let stats = svc.get_bin_inventory_stats(&service_ctx, &mut conn, path.id).await.ok();

 let content = bin_detail_page(&bww, zone, stats.as_ref());
 let detail_path_str = BinDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 库位详情", bww.bin.code),
 &claims,
 "inventory",
 &detail_path_str,
 "库存管理",
 Some(&bww.bin.code),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn temperature_label(req: &str) -> &str {
 match req {
 "ambient" => "常温",
 "cool" => "冷藏 (2~8°C)",
 "freeze" => "冷冻 (-18°C以下)",
 "constant" => "恒温",
 _ => "无要求",
 }
}

fn product_type_label(t: &str) -> &str {
 match t {
 "raw_material" => "原材料",
 "semi_finished" => "半成品",
 "finished" => "成品",
 "packaging" => "包材",
 "consumable" => "耗材",
 _ => t,
 }
}

fn product_type_color(t: &str) -> (&str, &str) {
 match t {
 "raw_material" => ("rgba(22,119,255,0.06)", "#1677ff"),
 "semi_finished" => ("rgba(82,196,26,0.06)", "#52c41a"),
 "finished" => ("rgba(114,46,209,0.06)", "#722ed1"),
 "packaging" => ("rgba(250,173,20,0.06)", "#d48806"),
 "consumable" => ("rgba(255,77,79,0.06)", "#ff4d4f"),
 _ => ("rgba(0,0,0,0.04)", "var(--muted)"),
 }
}

fn capacity_percent(stats: &BinInventoryStats, limit: Option<Decimal>) -> Option<Decimal> {
 limit.filter(|l| *l > Decimal::ZERO)
 .map(|l| (stats.total_quantity / l * Decimal::from(100)).min(Decimal::from(100)))
}

// ── Components ──

fn bin_detail_page(
 bww: &BinWithWarehouse,
 zone: Option<&Zone>,
 stats: Option<&BinInventoryStats>,
) -> Markup {
 let bin = &bww.bin;
 let status_label = bin_status_label(&bin.status);
 let status_class = bin_status_class(&bin.status);
 let _detail_path = BinDetailPath { id: bin.id };

 let zone_name = zone.map(|z| z.name.as_str()).unwrap_or("—");
 let _zone_code = zone.map(|z| z.code.as_str()).unwrap_or("—");

 let (used_qty, capacity_pct) = match stats {
 Some(s) => {
 let pct = capacity_percent(s, bin.capacity_limit);
 (format!("{:.2}", s.total_quantity), pct)
 }
 None => ("—".to_string(), None),
 };

 html! {
 div {
 // ── Back Link ──
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", BinListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回库位管理列表"
 }

 // ── Detail Header ──
 div class="block bg-bg border border-border-soft rounded-lg p-6" class="flex justify-between" class="items-start mb-5" {
 div {
 div class="flex items-center" class="gap-3" {
 h1 class="text-2xl font-extrabold" class="font-bold m-0 font-mono" class="text-xl" {
 (bin.code)
 }
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 div class="text-[13px] text-muted" class="mt-2" {
 (bww.warehouse_name) " · " (zone_name)
 }
 }
 }

 // ── Tabs ──
 div class="flex [border-bottom:1px_solid_var(--border-soft)]" class="flex" class="mb-5" style="gap:var(--space-1);border-bottom:1px solid var(--border-soft);padding-bottom:0" {
 button class="p-3 text-sm text-muted border-none cursor-pointer whitespace-nowrap font-medium active" class="cursor-pointer border-none bg-transparent text-accent" class="text-sm" style="padding:var(--space-2) var(--space-4);border-bottom:2px solid var(--accent)" onclick="switchTab('info',this)" { "基本信息" }
 button class="p-3 text-sm text-muted border-none cursor-pointer whitespace-nowrap font-medium" class="cursor-pointer border-none bg-transparent text-muted" class="text-sm" style="padding:var(--space-2) var(--space-4);border-bottom:2px solid transparent" onclick="switchTab('stock',this)" { "库存明细" }
 button class="p-3 text-sm text-muted border-none cursor-pointer whitespace-nowrap font-medium" class="cursor-pointer border-none bg-transparent text-muted" class="text-sm" style="padding:var(--space-2) var(--space-4);border-bottom:2px solid transparent" onclick="switchTab('history',this)" { "操作历史" }
 }

 // ── Tab: 基本信息 ──
 div.tab-panel id="tab-info" {
 // Info card
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "库位信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "库位编码" }
 span class="text-sm text-fg font-medium" class="font-mono" { (bin.code) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "库位名称" }
 span class="text-sm text-fg font-medium" { (bin.name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "所属仓库" }
 span class="text-sm text-fg font-medium" { (bww.warehouse_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "所属库区" }
 span class="text-sm text-fg font-medium" { (zone_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "库位状态" }
 span class="text-sm text-fg font-medium" {
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_label) }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "容量上限" }
 span class="text-sm text-fg font-medium" class="font-mono" {
 @if let Some(cap) = &bin.capacity_limit {
 (format!("{:.2}", cap))
 } @else {
 "—"
 }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "已用容量" }
 span class="text-sm text-fg font-medium" class="font-mono" class="text-warn" { (used_qty) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "温控要求" }
 span class="text-sm text-fg font-medium" {
 (bin.temperature_req.as_deref().map(temperature_label).unwrap_or("无要求"))
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "允许物料类型" }
 span class="text-sm text-fg font-medium" {
 @if let Some(types) = &bin.allowed_product_types {
 @for t in types {
 @let (bg, fg) = product_type_color(t);
 span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap" style=(format!("background:{bg};color:{fg};margin-right:4px")) {
 (product_type_label(t))
 }
 }
 } @else {
 "—"
 }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "创建时间" }
 span class="text-sm text-fg font-medium" class="font-mono" {
 (bin.created_at.format("%Y-%m-%d %H:%M"))
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "最后更新" }
 span class="text-sm text-fg font-medium" class="font-mono" {
 (bin.updated_at.format("%Y-%m-%d %H:%M"))
 }
 }
 }
 }

 // Coordinates card
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" class="mt-4" {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "库位坐标" }
 div class="flex" class="gap-4" class="mt-3" {
 div class="text-center flex-1 bg-surface" class="rounded-md p-4" class="border border-border-soft" {
 div class="font-bold font-mono text-fg" class="text-xl" {
 (bin.row_no.as_deref().unwrap_or("—"))
 }
 div class="text-muted" class="text-xs mt-1" { "行号 (Row)" }
 }
 div class="text-center flex-1 bg-surface" class="rounded-md p-4" class="border border-border-soft" {
 div class="font-bold font-mono text-fg" class="text-xl" {
 (bin.column_no.as_deref().unwrap_or("—"))
 }
 div class="text-muted" class="text-xs mt-1" { "列号 (Column)" }
 }
 div class="text-center flex-1 bg-surface" class="rounded-md p-4" class="border border-border-soft" {
 div class="font-bold font-mono text-fg" class="text-xl" {
 (bin.layer_no.as_deref().unwrap_or("—"))
 }
 div class="text-muted" class="text-xs mt-1" { "层号 (Layer)" }
 }
 div class="text-center flex-1 bg-surface" class="rounded-md p-4" class="border border-border-soft" {
 div class="font-bold font-mono text-fg" class="text-xl" {
 @if let Some(pct) = capacity_pct {
 (format!("{}%", pct.round()))
 } @else {
 "—"
 }
 }
 div class="text-muted" class="text-xs mt-1" { "容量使用率" }
 }
 }
 @if let Some(pct) = capacity_pct {
 div class="mt-4" style="max-width:400px" {
 div class="overflow-hidden" style="height:8px;background:var(--border-soft);border-radius:4px" {
 div style=(format!("width:{}%;background:var(--warn);height:100%;border-radius:4px;transition:width 0.3s", pct.round())) {}
 }
 }
 }
 }
 }

 // ── Tab: 库存明细 ──
 div.tab-panel id="tab-stock" style="display:none" {
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "产品编码" }
 th { "产品名称" }
 th { "批次号" }
 th class="text-right text-[13px]" { "数量" }
 th class="text-right text-[13px]" { "单位成本" }
 th { "入库日期" }
 th { "有效期" }
 th { "状态" }
 }
 }
 tbody {
 tr {
 td colspan="8" class="text-center text-muted py-8" {
 "暂无库存数据"
 }
 }
 }
 }
 }
 }
 }

 // ── Tab: 操作历史 ──
 div.tab-panel id="tab-history" style="display:none" {
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "时间" }
 th { "事务类型" }
 th { "关联单号" }
 th { "产品" }
 th class="text-right text-[13px]" { "变动数量" }
 th { "操作员" }
 th { "备注" }
 }
 }
 tbody {
 tr {
 td colspan="7" class="text-center text-muted py-8" {
 "暂无操作历史"
 }
 }
 }
 }
 }
 }
 }

 // ── Tab switch script ──
 script {
 r#"
 function switchTab(tabId, btn) {
 document.querySelectorAll('.tab-panel').forEach(function(p) {
 p.style.display = 'none';
 });
 document.querySelectorAll('.detail-tab').forEach(function(t) {
 t.style.color = 'var(--muted)';
 t.style.borderBottomColor = 'transparent';
 });
 document.getElementById('tab-' + tabId).style.display = '';
 btn.style.color = 'var(--accent)';
 btn.style.borderBottomColor = 'var(--accent)';
 }
 "#
 }
 }
 }
}
