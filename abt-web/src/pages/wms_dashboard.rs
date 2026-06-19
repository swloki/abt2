use axum::response::Html;
use maud::{html, Markup};

use abt_core::shared::types::PageParams;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::stock_ledger::model::StockFilter;
use abt_core::wms::warehouse::{WarehouseFilter, WarehouseService};

use chrono::Datelike;
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_dashboard::WmsDashboardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
// ── Handler ──

#[require_permission("INVENTORY", "read")]
pub async fn get_wms_dashboard(
 _path: WmsDashboardPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let wh_svc = state.warehouse_service();
 let inv_svc = state.inventory_service();
 let txn_svc = state.inventory_transaction_service();

 let page1 = PageParams::new(1, 1);

 // 仓库总数
 let warehouse_count = wh_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), page1.page, page1.page_size)
 .await
 .map(|r| r.total)
 .unwrap_or(0);

 // 库存品类数（query_stock total）
 let stock_sku_count = txn_svc
 .query_stock(
 &service_ctx,
 &mut conn,
 StockFilter::default(),
 page1.page,
 page1.page_size,
 )
 .await
 .map(|r| r.total)
 .unwrap_or(0);

 // 低库存预警
 let low_stock_count = inv_svc
 .list_low_stock(&service_ctx, &mut conn)
 .await
 .map(|v| v.len() as u64)
 .unwrap_or(0);

 // 本月入库/出库 — 使用 query_logs 按月过滤
 let now = chrono::Utc::now();
 let month_start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
 .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
 .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
 .unwrap_or(now);

 let in_filter = abt_core::wms::inventory::model::TransactionLogFilter {
 start_date: Some(month_start),
 transaction_type: Some("PURCHASE_RECEIPT".into()),
 ..Default::default()
 };

 let month_in_count = inv_svc
 .query_logs(&service_ctx, &mut conn, in_filter, page1.page, page1.page_size)
 .await
 .map(|r| r.total)
 .unwrap_or(0);

 let out_filter = abt_core::wms::inventory::model::TransactionLogFilter {
 start_date: Some(month_start),
 transaction_type: Some("SALES_SHIPMENT".into()),
 ..Default::default()
 };

 let month_out_count = inv_svc
 .query_logs(&service_ctx, &mut conn, out_filter, page1.page, page1.page_size)
 .await
 .map(|r| r.total)
 .unwrap_or(0);

 let stats = DashboardStats {
 warehouse_count,
 stock_sku_count,
 month_in_count,
 month_out_count,
 low_stock_count,
 };

 let content = wms_dashboard_content(&stats);
 let page_html = admin_page(
 is_htmx,
 "库存管理总览",
 &claims,
 "inventory",
 "/admin/wms",
 "库存管理",
 None,
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

// ── Data ──

struct DashboardStats {
 warehouse_count: u64,
 stock_sku_count: u64,
 month_in_count: u64,
 month_out_count: u64,
 low_stock_count: u64,
}

// ── Main content (matches prototype 03-index.html) ──

const BTN_DEFAULT: &str =
 "inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border \
  hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium \
  cursor-pointer transition-all duration-150 shadow-xs";

fn stat_card(icon: &Markup, icon_cls: &str, value: &str, label: &str) -> Markup {
 html! {
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md shadow-xs" {
 div class=(format!("w-[44px] h-[44px] rounded-md grid place-items-center shrink-0 {}", icon_cls)) {
 (icon)
 }
 div {
 div class="text-2xl font-bold font-mono tabular-nums text-fg" { (value) }
 div class="text-sm text-muted mt-1" { (label) }
 }
 }
 }
}

fn wms_dashboard_content(stats: &DashboardStats) -> Markup {
 html! {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "库存管理总览" }
 button class=(BTN_DEFAULT) {
 (icon::download_icon("w-4 h-4"))
 "导出报表"
 }
 }

 // ── Stat Cards ──
 div class="grid grid-cols-5 gap-5 mb-8" {
 (stat_card(&icon::building_icon("w-[22px] h-[22px]"), "bg-accent-bg text-accent", &stats.warehouse_count.to_string(), "仓库总数"))
 (stat_card(&icon::box_icon("w-[22px] h-[22px]"), "bg-success-bg text-success", &format_number(stats.stock_sku_count), "库存品类"))
 (stat_card(&icon::download_icon("w-[22px] h-[22px]"), "bg-accent-bg text-accent", &stats.month_in_count.to_string(), "本月入库"))
 (stat_card(&icon::upload_icon("w-[22px] h-[22px]"), "bg-danger-bg text-danger", &stats.month_out_count.to_string(), "本月出库"))
 (stat_card(&icon::circle_alert_icon("w-[22px] h-[22px]"), "bg-danger-bg text-danger", &stats.low_stock_count.to_string(), "低库存预警"))
 }

 // ── Quick Entry Grid ──
 div class="mb-8" {
 h2 class="text-lg font-semibold text-fg mb-4" { "快捷入口" }
 div class="grid grid-cols-4 gap-4" {
 (quick_entry_card("/admin/wms/warehouses", "bg-accent-bg", "text-accent", &icon::building_icon("w-[22px] h-[22px]"), "仓库管理", "仓库主数据与分区配置"))
 (quick_entry_card("/admin/wms/bins", "bg-success-bg", "text-success", &icon::grid_icon("w-[22px] h-[22px]"), "库位管理", "库位规划与容量管理"))
 (quick_entry_card("/admin/wms/stock", "bg-accent-bg", "text-accent", &icon::search_icon("w-[22px] h-[22px]"), "库存查询", "实时库存数量与批次"))
 (quick_entry_card("/admin/wms/arrivals", "bg-warn-bg", "text-warn", &icon::truck_icon("w-[22px] h-[22px]"), "来料通知", "供应商送货到货登记"))
 (quick_entry_card("/admin/wms/stock-in", "bg-accent-bg", "text-accent", &icon::download_icon("w-[22px] h-[22px]"), "入库管理", "采购入库 / 生产入库"))
 (quick_entry_card("/admin/wms/stock-out", "bg-danger-bg", "text-danger", &icon::upload_icon("w-[22px] h-[22px]"), "出库管理", "销售出库 / 生产领料"))
 (quick_entry_card("/admin/wms/requisitions", "bg-success-bg", "text-success", &icon::clipboard_module_icon("w-[22px] h-[22px]"), "领料单", "生产领料与发料管理"))
 (quick_entry_card("/admin/wms/cycle-counts", "bg-warn-bg", "text-warn", &icon::clipboard_list_icon("w-[22px] h-[22px]"), "循环盘点", "定期盘点与差异处理"))
 (quick_entry_card("/admin/wms/transfers", "bg-accent-bg", "text-accent", &icon::arrow_right_icon("w-[22px] h-[22px]"), "库存调拨", "跨仓调拨与在途管理"))
 (quick_entry_card("/admin/wms/conversions", "bg-danger-bg", "text-danger", &icon::refresh_icon("w-[22px] h-[22px]"), "形态转换", "物料形态与单位转换"))
 (quick_entry_card("/admin/wms/backflushes", "bg-success-bg", "text-success", &icon::refresh_icon("w-[22px] h-[22px]"), "倒冲记录", "生产完工自动扣料"))
 (quick_entry_card("/admin/wms/locks", "bg-danger-bg", "text-danger", &icon::lock_icon("w-[22px] h-[22px]"), "库存锁定", "质检与预留库存冻结"))
 (quick_entry_card("/admin/wms/transactions", "bg-warn-bg", "text-warn", &icon::file_text_icon("w-[22px] h-[22px]"), "事务日志", "全量库存事务流水"))
 (quick_entry_card("/admin/wms/strategies", "bg-accent-bg", "text-accent", &icon::sliders_icon("w-[22px] h-[22px]"), "策略管理", "上架与拣货策略配置"))
 }
 }

 // ── Recent Operations ──
 div {
 h2 class="text-lg font-semibold text-fg mb-4" { "最近操作" }
 div class="data-card overflow-hidden" {
 table class="data-table w-full" {
 thead {
 tr {
 th { "时间" }
 th { "操作类型" }
 th { "单号" }
 th { "仓库" }
 th { "操作人" }
 }
 }
 tbody {
 tr {
 td class="text-muted text-xs" { "—" }
 td { span class="inline-flex items-center gap-1 rounded-full text-xs font-medium whitespace-nowrap bg-[#e8f4ff] text-accent" { "来料接收" } }
 td { a href="/admin/wms/arrivals" class="text-accent" { "—" } }
 td { "—" }
 td { "—" }
 }
 tr {
 td class="text-muted text-xs" { "—" }
 td { span class="inline-flex items-center gap-1 rounded-full text-xs font-medium whitespace-nowrap bg-warn-bg text-warn" { "领料出库" } }
 td { a href="/admin/wms/requisitions" class="text-accent" { "—" } }
 td { "—" }
 td { "—" }
 }
 tr {
 td class="text-muted text-xs" { "—" }
 td { span class="inline-flex items-center gap-1 rounded-full text-xs font-medium whitespace-nowrap bg-success-bg text-success" { "库存调拨" } }
 td { a href="/admin/wms/transfers" class="text-accent" { "—" } }
 td { "—" }
 td { "—" }
 }
 tr {
 td class="text-muted text-xs" { "—" }
 td { span class="inline-flex items-center gap-1 rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted" { "循环盘点" } }
 td { a href="/admin/wms/cycle-counts" class="text-accent" { "—" } }
 td { "—" }
 td { "—" }
 }
 tr {
 td class="text-muted text-xs" { "—" }
 td { span class="inline-flex items-center gap-1 rounded-full text-xs font-medium whitespace-nowrap bg-warn-bg text-warn" { "库存锁定" } }
 td { a href="/admin/wms/locks" class="text-accent" { "—" } }
 td { "—" }
 td { "—" }
 }
 }
 }
 }
 }
 }
}

// ── Sub-components ──

fn format_number(n: u64) -> String {
 if n >= 1_000_000 {
 format!("{:.1}M", n as f64 / 1_000_000.0)
 } else if n >= 10_000 {
 format!("{:.0}K", n as f64 / 1_000.0)
 } else {
 n.to_string()
 }
}

fn quick_entry_card(
 href: &str,
 icon_bg: &str,
 icon_color: &str,
 svg_icon: &Markup,
 title: &str,
 desc: &str,
) -> Markup {
 html! {
 a href=(href) class="flex flex-col items-center gap-3 rounded-md border border-border-soft px-4 py-6 bg-bg shadow-xs no-underline cursor-pointer hover:border-accent hover:bg-accent-bg transition-colors" {
 div class=(format!("w-11 h-11 rounded-md grid place-items-center {}", icon_bg)) {
 span class=(icon_color) { (svg_icon) }
 }
 span class="text-sm font-semibold text-fg" { (title) }
 span class="text-xs text-muted text-center" { (desc) }
 }
 }
}

