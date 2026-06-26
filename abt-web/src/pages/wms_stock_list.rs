use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::Deserialize;

use abt_core::master_data::product::{ProductQuery, ProductService};
use abt_core::shared::types::PageParams;
use abt_core::wms::stock_ledger::model::StockFilter;
use abt_core::wms::stock_ledger::StockLedgerService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::warehouse::model::WarehouseFilter;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock::{StockListPath, StockZonesPath, StockDetailPath, StockDetailQuery};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct StockQueryParams {
 pub product_code: Option<String>,
 pub product_name: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub low_stock: Option<bool>,
 pub page: Option<u32>,
 pub batch_no: Option<String>,
}

// ── Helpers ──

fn build_filter(params: &StockQueryParams, single_product_id: Option<i64>, product_ids: Vec<i64>) -> StockFilter {
 StockFilter {
 product_id: single_product_id,
 product_ids: if !product_ids.is_empty() && single_product_id.is_none() { Some(product_ids) } else { None },
 warehouse_id: params.warehouse_id,
 zone_id: params.zone_id,
 bin_id: None,
 batch_no: params.batch_no.as_deref().filter(|s| !s.is_empty()).map(String::from),
 }
}

/// Look up product IDs matching the code/name search terms.
/// Returns (single_id_for_filter, all_matching_ids_for_post_filter).
/// - If exactly one product matches → single_id = Some(id), use it in StockFilter for efficient DB query.
/// - If zero or multiple match → single_id = None, post-filter by all_matching_ids.
/// - If no search terms → both empty, meaning "all products".
async fn resolve_product_search(
 product_svc: &impl ProductService,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 params: &StockQueryParams,
) -> Result<(Option<i64>, Vec<i64>)> {
 let has_code = params.product_code.as_deref().is_some_and(|s| !s.trim().is_empty());
 let has_name = params.product_name.as_deref().is_some_and(|s| !s.trim().is_empty());

 if !has_code && !has_name {
 return Ok((None, vec![]));
 }

 let query = ProductQuery {
 code: if has_code { params.product_code.clone() } else { None },
 name: if has_name { params.product_name.clone() } else { None },
 ..Default::default()
 };
 let result = product_svc.list(ctx, db, query, PageParams { page: 1, page_size: 200 }).await?;
 let ids: Vec<i64> = result.items.iter().map(|p| p.product_id).collect();


 if ids.len() == 1 {
 Ok((Some(ids[0]), ids))
 } else {
 Ok((None, ids))
 }
}

async fn resolve_product_names<S: ProductService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::wms::stock_ledger::model::StockLedger],
) -> HashMap<i64, (String, String)> {
 let unique_ids: Vec<i64> = items.iter().map(|s| s.product_id).collect();
 if unique_ids.is_empty() {
 return HashMap::new();
 }
 match svc.get_by_ids(ctx, db, unique_ids).await {
 Ok(products) => products
 .into_iter()
 .map(|p| (p.product_id, (p.product_code, p.pdt_name)))
 .collect(),
 Err(_) => HashMap::new(),
 }
}

async fn resolve_warehouse_names<S: WarehouseService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::wms::stock_ledger::model::StockLedger],
) -> HashMap<i64, String> {
 let unique_ids: Vec<i64> = items.iter().map(|s| s.warehouse_id).collect();
 if unique_ids.is_empty() {
 return HashMap::new();
 }
 let mut map = HashMap::new();
 for id in unique_ids {
 if !map.contains_key(&id)
 && let Ok(wh) = svc.get(ctx, db, id).await {
 map.insert(id, wh.name);
 }
 }
 map
}

async fn resolve_zone_codes<S: WarehouseService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::wms::stock_ledger::model::StockLedger],
) -> HashMap<i64, String> {
 let warehouse_ids: Vec<i64> = items.iter().map(|s| s.warehouse_id).collect();
 let mut map = HashMap::new();
 for wid in warehouse_ids {
 if let Ok(zones) = svc.list_zones(ctx, db, wid).await {
 for z in zones {
 map.insert(z.id, z.code);
 }
 }
 }
 map
}

async fn resolve_bin_codes<S: WarehouseService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::wms::stock_ledger::model::StockLedger],
) -> HashMap<i64, String> {
 let zone_ids: Vec<i64> = items.iter().map(|s| s.zone_id).collect();
 let mut map = HashMap::new();
 for zid in zone_ids {
 if !map.contains_key(&zid)
 && let Ok(result) = svc.list_bins(ctx, db, zid, None, 1, 500).await {
 for b in result.items {
 map.insert(b.id, b.code);
 }
 }
 }
 map
}

fn format_decimal(d: &Decimal) -> String {
 fn fmt_int(n: i64) -> String {
 let s = n.abs().to_string();
 let mut parts: Vec<&str> = s.as_bytes().rchunks(3).map(|c| std::str::from_utf8(c).unwrap()).collect();
 parts.reverse();
 let joined = parts.join(",");
 if n < 0 { format!("-{joined}") } else { joined }
 }
 let v: f64 = d.to_f64().unwrap_or(0.0);
 if v == (v as i64) as f64 {
 fmt_int(v as i64)
 } else {
 format!("{v:.2}")
 }
}

// ── Handlers ──

/// HTMX: 返回库区选项（根据 warehouse_id 级联）
#[derive(Debug, Deserialize)]
pub struct ZoneCascadeParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
}

pub async fn get_zone_options(
 _path: StockZonesPath,
 ctx: RequestContext,
 Query(params): Query<ZoneCascadeParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let zones = if let Some(wid) = params.warehouse_id {
 warehouse_svc.list_zones(&service_ctx, &mut conn, wid).await.unwrap_or_default()
 } else {
 vec![]
 };

 Ok(Html(zone_select_fragment(&zones, params.zone_id).into_string()))
}

fn zone_select_fragment(
 zones: &[abt_core::wms::warehouse::model::Zone],
 selected: Option<i64>,
) -> Markup {
 html! {
    select
        class="w-[140px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
        name="zone_id"
        id="zone-select"
    {
        option value="" { "全部库区" }
        @for z in zones {
            option value=(z.id) selected[selected == Some(z.id)] { (z.code) }
        }
    }
}
}

#[require_permission("INVENTORY", "read")]
pub async fn get_stock_list(
 _path: StockListPath,
 ctx: RequestContext,
 Query(params): Query<StockQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.stock_ledger_service();
 let product_svc = state.product_service();
 let warehouse_svc = state.warehouse_service();

 let has_search = params.product_code.as_deref().is_some_and(|s| !s.trim().is_empty())
 || params.product_name.as_deref().is_some_and(|s| !s.trim().is_empty());
 let (single_pid, all_pids) = resolve_product_search(&product_svc, &service_ctx, &mut conn, &params).await.unwrap_or((None, vec![]));
 let filter = build_filter(&params, single_pid, all_pids.clone());
 let page_num = params.page.unwrap_or(1);
 let mut result = svc.query(&service_ctx, &mut conn, filter, page_num, 20).await?;

 // 搜索了但没找到匹配产品 → 清空结果
 if has_search && all_pids.is_empty() && single_pid.is_none() {
 result.items.clear();
 }

 let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &result.items).await;
 let warehouses = warehouse_svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await.map(|r| r.items).unwrap_or_default();
 let zones = if let Some(wid) = params.warehouse_id {
 warehouse_svc.list_zones(&service_ctx, &mut conn, wid).await.unwrap_or_default()
 } else {
 vec![]
 };
 let ctx = StockListContext { product_names: &product_names, warehouses: &warehouses, zones: &zones, params: &params };
 let content = stock_list_page(&result, &ctx);
 let page_html = admin_page(
 is_htmx, "库存查询", &claims, "inventory", StockListPath::PATH, "库存管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

// ── Components ──
struct StockListContext<'a> {
 product_names: &'a HashMap<i64, (String, String)>,
 warehouses: &'a [abt_core::wms::warehouse::model::Warehouse],
 zones: &'a [abt_core::wms::warehouse::model::Zone],
 params: &'a StockQueryParams,
}

fn stat_card(icon_markup: &Markup, icon_cls: &str, value: &str, label: &str, danger: bool) -> Markup {
 let vcls = if danger { "text-danger" } else { "text-fg" };
 html! {
    div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md shadow-xs" {
        div class=({
                format!(
                    "w-[44px] h-[44px] rounded-md grid place-items-center shrink-0 {}",
                    icon_cls,
                )
            })
        { (icon_markup) }
        div {
            div class=(format!("text-2xl font-bold font-mono tabular-nums {}", vcls)) { (value) }
            div class="text-sm text-muted mt-1" { (label) }
        }
    }
}
}

fn stock_list_page(
 result: &abt_core::shared::types::PaginatedResult<abt_core::wms::stock_ledger::model::StockLedger>,
 ctx: &StockListContext,
) -> Markup {
 html! {
    div {
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "库存查询" }
        }
        // ── Stat Cards ──
        div class="grid grid-cols-4 gap-4 mb-6" {
            ({
                stat_card(
                    &icon::box_icon("w-5 h-5"),
                    "bg-accent-bg text-accent",
                    &result
                        .items
                        .iter()
                        .map(|s| s.product_id)
                        .collect::<HashSet<_>>()
                        .len()
                        .to_string(),
                    "总品种",
                    false,
                )
            })
            ({
                stat_card(
                    &icon::package_icon("w-5 h-5"),
                    "bg-success-bg text-success",
                    &format_decimal(
                        &result.items.iter().map(|s| s.quantity).sum::<Decimal>(),
                    ),
                    "总库存量",
                    false,
                )
            })
            ({
                stat_card(
                    &icon::circle_alert_icon("w-5 h-5"),
                    "bg-warn-bg text-warn",
                    &result
                        .items
                        .iter()
                        .filter(|s| s.available_qty <= Decimal::ZERO)
                        .count()
                        .to_string(),
                    "低库存项",
                    true,
                )
            })
            ({
                stat_card(
                    &icon::lock_icon("w-5 h-5"),
                    "bg-danger-bg text-danger",
                    &format_decimal(
                        &result.items.iter().map(|s| s.reserved_qty).sum::<Decimal>(),
                    ),
                    "已预留量",
                    false,
                )
            })
        }
        // ── Filter Bar (outside data-card, always visible) ──
        (stock_filter_bar(ctx.warehouses, ctx.zones, ctx.params))
        // ── Data Card (HTMX target) ──
        (stock_data_card(result, ctx.product_names))
        // ── Detail Drawer ──
        ({
            crate::components::drawer::drawer_with_footer(
                "stock-drawer",
                "库存详情",
                html! {
                    div id = "stock-drawer-content" {}
                },
                html! {
                    button type = "button" class =
                    "inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    _ = "on click remove .open from closest .drawer-overlay" { "关闭" }
                },
            )
        })
    }
}
}

/// Filter bar with separate product_code / product_name inputs.
/// Uses the same pattern as product_list: form-level hx-trigger with debounce.
fn stock_filter_bar(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 zones: &[abt_core::wms::warehouse::model::Zone],
 params: &StockQueryParams,
) -> Markup {
 html! {
    form
        class="flex items-center gap-3 mb-5 flex-wrap"
        id="stock-filter-form"
        hx-get=(StockListPath::PATH)
        hx-trigger="change, keyup changed delay:300ms from:.search-input"
        hx-target="#stock-data-card"
        hx-select="#stock-data-card"
        hx-swap="outerHTML"
        hx-include="#stock-filter-form"
       
    {
        div class="relative w-[180px] icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
        {
            (icon::search_icon(""))
            input
                class="search-input w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                type="text"
                name="product_code"
                placeholder="产品编码"
                value=(params.product_code.as_deref().unwrap_or("")) {}
        }
        div class="relative w-[180px] icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
        {
            (icon::search_icon(""))
            input
                class="search-input w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                type="text"
                name="product_name"
                placeholder="产品名称"
                value=(params.product_name.as_deref().unwrap_or("")) {}
        }
        select
            class="w-[160px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
            name="warehouse_id"
            hx-get=(StockZonesPath::PATH)
            hx-trigger="change"
            hx-target="#zone-select"
            hx-swap="outerHTML"
            hx-include="[name='zone_id']"
        {
            option value="" { "全部仓库" }
            @for w in warehouses {
                option value=(w.id) selected[params.warehouse_id == Some(w.id)] { (w.name) }
            }
        }
        (zone_select_fragment(zones, params.zone_id))
        div class="relative w-[180px] icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
        {
            (icon::search_icon(""))
            input
                class="search-input w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                type="text"
                name="batch_no"
                placeholder="批次号"
                value=(params.batch_no.as_deref().unwrap_or("")) {}
        }
        label class="flex items-center gap-2 text-sm text-fg-2 cursor-pointer whitespace-nowrap" {
            input
                type="checkbox"
                name="low_stock"
                value="true"
                class="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                checked[params.low_stock == Some(true)] {}
            "仅显示低库存"
        }
    }
}
}

/// The data-card with table + pagination. This is the HTMX swap target.
fn stock_data_card(
 result: &abt_core::shared::types::PaginatedResult<abt_core::wms::stock_ledger::model::StockLedger>,
 product_names: &HashMap<i64, (String, String)>,
) -> Markup {
 html! {
    div id="stock-data-card" class="data-card" {
        div class="overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th { "产品编码" }
                        th { "产品名称" }
                        th class="text-right text-[13px]" { "现有量" }
                        th class="text-right text-[13px]" { "可用量" }
                        th class="!text-right" { "操作" }
                    }
                }
                tbody {
                    @for item in &result.items {
                        @let product_info = product_names.get(&item.product_id);
                        @let p_code = product_info
                            .map(|(c, _)| c.as_str())
                            .unwrap_or("—");
                        @let p_name = product_info
                            .map(|(_, n)| n.as_str())
                            .unwrap_or("—");
                        @let is_low = item.available_qty <= Decimal::ZERO;
                        @let code_cls = if is_low {
                            "text-danger font-medium cursor-pointer font-mono tabular-nums"
                        } else {
                            "text-accent font-medium cursor-pointer font-mono tabular-nums"
                        };
                        @let name_cls = if is_low { "text-danger" } else { "text-fg" };
                        @let avail_cls = if is_low {
                            "text-right text-[13px] text-danger font-semibold"
                        } else {
                            "text-right text-[13px] text-fg"
                        };
                        tr {
                            td class=(code_cls) { (p_code) }
                            td class=(name_cls) { (p_name) }
                            td class="text-right text-[13px] text-fg" {
                                (format_decimal(&item.quantity))
                            }
                            td class=(avail_cls) { (format_decimal(&item.available_qty)) }
                            td {
                                a   class="text-accent text-sm cursor-pointer"
                                    hx-get=(format!("{}?id={}", StockDetailPath::PATH, item.id))
                                    hx-target="#stock-drawer-content"
                                    hx-swap="innerHTML"
                                    _="on 'htmx:afterRequest' add .open to #stock-drawer"
                                { "详情" }
                            }
                        }
                    }
                    @if result.items.is_empty() {
                        tr {
                            td colspan="5" class="text-center text-muted py-8" { "暂无库存数据" }
                        }
                    }
                }
            }
        }
        ({
            pagination(
                StockListPath::PATH,
                "#stock-data-card",
                "#stock-filter-form",
                result.total,
                result.page,
                result.total_pages,
            )
        })
    }
}
}

// ── Stock Detail Drawer (HTMX endpoint) ──

#[require_permission("INVENTORY", "read")]
pub async fn get_stock_detail(
 _path: StockDetailPath,
 ctx: RequestContext,
 Query(query): Query<StockDetailQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.stock_ledger_service();
 let product_svc = state.product_service();
 let warehouse_svc = state.warehouse_service();

 let filter = StockFilter { product_id: None, product_ids: None, warehouse_id: None, zone_id: None, bin_id: None, batch_no: None };
 let result = svc.query(&service_ctx, &mut conn, filter, 1, 10000).await?;

 let item = result.items.iter().find(|i| i.id == query.id);
 let Some(item) = item else {
 return Ok(Html(r#"<div class="p-6 text-sm text-muted">未找到库存记录</div>"#.into()));
 };

 let product_info = resolve_product_names(&product_svc, &service_ctx, &mut conn, std::slice::from_ref(item)).await;
 let warehouse_name = resolve_warehouse_names(&warehouse_svc, &service_ctx, &mut conn, std::slice::from_ref(item)).await;
 let zone_code = resolve_zone_codes(&warehouse_svc, &service_ctx, &mut conn, std::slice::from_ref(item)).await;
 let bin_code = resolve_bin_codes(&warehouse_svc, &service_ctx, &mut conn, std::slice::from_ref(item)).await;

 let p_code = product_info.get(&item.product_id).map(|(c, _)| c.clone()).unwrap_or_default();
 let p_name = product_info.get(&item.product_id).map(|(_, n)| n.clone()).unwrap_or_default();
 let wh = warehouse_name.get(&item.warehouse_id).cloned().unwrap_or_else(|| "—".into());
 let zone = zone_code.get(&item.zone_id).cloned().unwrap_or_else(|| "—".into());
 let bin = bin_code.get(&item.bin_id).cloned().unwrap_or_else(|| "—".into());

 Ok(Html(stock_detail_content(item, &p_code, &p_name, &wh, &zone, &bin).into_string()))
}

fn stock_detail_content(
 item: &abt_core::wms::stock_ledger::model::StockLedger,
 p_code: &str,
 p_name: &str,
 warehouse: &str,
 zone: &str,
 bin: &str,
) -> Markup {
 let safe_stock = Decimal::ZERO; // TODO: 从产品安全库存读取
 let pct = if safe_stock > Decimal::ZERO {
 std::cmp::min(100, (item.available_qty / safe_stock * Decimal::from(100)).to_u32().unwrap_or(100))
 } else { 100 };
 let bar_color = if pct < 25 { "var(--danger)" } else if pct < 60 { "var(--warn)" } else { "var(--success)" };
 let stock_value = item.unit_cost.map(|c| item.quantity * c);
 let is_low = item.available_qty <= Decimal::ZERO;

 html! {
    div class="mb-6 last:mb-0" {
        div class="text-xs font-semibold text-fg-2 mb-3" { "基本信息" }
        div class="grid grid-cols-2 gap-4" {
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "产品编码" }
                span class="font-mono tabular-nums text-sm text-fg" { (p_code) }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "产品名称" }
                span class="text-sm text-fg" { (p_name) }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "仓库" }
                span class="text-sm text-fg" { (warehouse) }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "库区" }
                span class="text-sm text-fg" { (zone) }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "库位" }
                span class="font-mono tabular-nums text-sm text-fg" { (bin) }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "批次号" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    (item.batch_no.as_deref().unwrap_or("—"))
                }
            }
        }
    }
    div class="mb-6 last:mb-0" {
        div class="text-xs font-semibold text-fg-2 mb-3" { "库存数量" }
        div class="grid grid-cols-2 gap-4" {
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "现有量" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    (format_decimal(&item.quantity))
                }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "已预留" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    (format_decimal(&item.reserved_qty))
                }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "可用量" }
                span
                    class=({
                        format!(
                            "font-mono tabular-nums text-sm {}",
                            if is_low { "text-danger font-semibold" } else { "text-fg" },
                        )
                    })
                { (format_decimal(&item.available_qty)) }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "安全库存" }
                span class="font-mono tabular-nums text-sm text-fg" { (format_decimal(&safe_stock)) }
            }
        }
        div class="mt-3" {
            div class="flex justify-between text-xs text-muted mb-1" {
                span { "可用量 / 安全库存" }
                span class="font-semibold" style=(format!("color:{bar_color}")) {
                    (format!("{pct}%"))
                }
            }
            div class="h-2 rounded-full bg-border overflow-hidden" {
                div class="h-full rounded-full"
                    style=(format!("width:{pct}%;background:{bar_color}")) {}
            }
        }
    }
    div class="mb-6 last:mb-0" {
        div class="text-xs font-semibold text-fg-2 mb-3" { "财务与日期" }
        div class="grid grid-cols-2 gap-4" {
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "单位成本" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    ({
                        item.unit_cost
                            .map(|c| format!("¥{}", format_decimal(&c)))
                            .unwrap_or_else(|| "—".into())
                    })
                }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "库存金额" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    ({
                        stock_value
                            .map(|v| format!("¥{}", format_decimal(&v)))
                            .unwrap_or_else(|| "—".into())
                    })
                }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "入库日期" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    ({
                        item.received_date
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "—".into())
                    })
                }
            }
            div class="flex flex-col gap-1" {
                label class="text-xs text-muted" { "有效期" }
                span class="font-mono tabular-nums text-sm text-fg" {
                    ({
                        item.expiry_date
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "—".into())
                    })
                }
            }
        }
    }
}
}
