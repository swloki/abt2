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
use crate::routes::wms_stock::{StockListPath, StockTablePath, StockZonesPath, StockDetailPath, StockDetailQuery};
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

fn build_query_string(params: &StockQueryParams) -> String {
    let mut q = vec![];
    if let Some(code) = &params.product_code
        && !code.is_empty() {
            q.push(format!("product_code={code}"));
        }
    if let Some(name) = &params.product_name
        && !name.is_empty() {
            q.push(format!("product_name={name}"));
        }
    if let Some(wid) = params.warehouse_id {
        q.push(format!("warehouse_id={wid}"));
    }
    if let Some(zid) = params.zone_id {
        q.push(format!("zone_id={zid}"));
    }
    if params.low_stock == Some(true) {
        q.push("low_stock=true".into());
    }
    if let Some(bn) = &params.batch_no
        && !bn.is_empty() {
            q.push(format!("batch_no={bn}"));
        }
    q.join("&")
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
        select class="form-select" name="zone_id" id="zone-select" style="width:140px" {
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

#[require_permission("INVENTORY", "read")]
pub async fn get_stock_table(
    _path: StockTablePath,
    ctx: RequestContext,
    Query(params): Query<StockQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.stock_ledger_service();
    let product_svc = state.product_service();


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
    // HTMX partial: return only the data-card
    Ok(Html(stock_data_card(&result, &product_names, &params).into_string()))
}

// ── Components ──
struct StockListContext<'a> {
    product_names: &'a HashMap<i64, (String, String)>,
    warehouses: &'a [abt_core::wms::warehouse::model::Warehouse],
    zones: &'a [abt_core::wms::warehouse::model::Zone],
    params: &'a StockQueryParams,
}

fn stock_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::stock_ledger::model::StockLedger>,
    ctx: &StockListContext,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "库存查询" }
            }

            // ── Stat Cards ──
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4);margin-bottom:var(--space-6)" {
                div class="stat-card" {
                    div class="stat-icon blue" {
                        (icon::box_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { (result.items.iter().map(|s| s.product_id).collect::<HashSet<_>>().len()) }
                        div class="stat-label" { "总品种" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon green" {
                        (icon::package_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { (format_decimal(&result.items.iter().map(|s| s.quantity).sum::<Decimal>())) }
                        div class="stat-label" { "总库存量" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        (icon::circle_alert_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" style="color:var(--danger)" { (result.items.iter().filter(|s| s.available_qty <= Decimal::ZERO).count()) }
                        div class="stat-label" { "低库存项" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon red" {
                        (icon::lock_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { (format_decimal(&result.items.iter().map(|s| s.reserved_qty).sum::<Decimal>())) }
                        div class="stat-label" { "已预留量" }
                    }
                }
            }

            // ── Filter Bar (outside data-card, always visible) ──
            (stock_filter_bar(ctx.warehouses, ctx.zones, ctx.params))
            // ── Data Card (HTMX target) ──
            (stock_data_card(result, ctx.product_names, ctx.params))

            // ── Detail Drawer ──
            (crate::components::drawer::drawer_with_footer(
                "stock-drawer",
                "库存详情",
                html! {
                    div id="stock-drawer-content" {}
                },
                html! {
                    button type="button" class="btn btn-default"
                        onclick="hsRemoveClosest(this,'.drawer-overlay','open')" { "关闭" }
                },
            ))
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
        form class="filter-bar filter-form"
            hx-get=(StockTablePath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#stock-data-card"
            hx-select="#stock-data-card"
            hx-swap="outerHTML"
            hx-include="closest form" {
            div class="search-wrap" {
                (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="product_code"
                    style="width:180px"
                    placeholder="产品编码"
                    value=(params.product_code.as_deref().unwrap_or(""));
            }
            div class="search-wrap" {
                (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="product_name"
                    placeholder="产品名称"
                    value=(params.product_name.as_deref().unwrap_or(""));
            }
            select class="form-select" name="warehouse_id" style="width:160px"
                hx-get=(StockZonesPath::PATH)
                hx-trigger="change"
                hx-target="#zone-select"
                hx-swap="outerHTML"
                hx-include="[name='zone_id']" {
                option value="" { "全部仓库" }
                @for w in warehouses {
                    option value=(w.id) selected[params.warehouse_id == Some(w.id)] { (w.name) }
                }
            }
            (zone_select_fragment(zones, params.zone_id))
            div class="search-wrap" {
                (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="batch_no"
                    placeholder="批次号"
                    value=(params.batch_no.as_deref().unwrap_or(""));
            }
            label class="toggle-wrap" style="cursor:pointer;display:flex;align-items:center;gap:var(--space-2);font-size:var(--text-sm);color:var(--fg-2);white-space:nowrap" {
                input type="checkbox" name="low_stock" value="true"
                    style="width:16px;height:16px;accent-color:var(--accent)"
                    checked[params.low_stock == Some(true)];
                "仅显示低库存"
            }
        }
    }
}

/// The data-card with table + pagination. This is the HTMX swap target.
fn stock_data_card(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::stock_ledger::model::StockLedger>,
    product_names: &HashMap<i64, (String, String)>,
    params: &StockQueryParams,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div id="stock-data-card" class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "产品编码" }
                            th { "产品名称" }
                            th class="num-right" { "现有量" }
                            th class="num-right" { "可用量" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let product_info = product_names.get(&item.product_id);
                            @let p_code = product_info.map(|(c, _)| c.as_str()).unwrap_or("—");
                            @let p_name = product_info.map(|(_, n)| n.as_str()).unwrap_or("—");
                            @let is_low = item.available_qty <= Decimal::ZERO;
                            @let low_class = if is_low { "text-low-stock" } else { "" };
                            @let danger_style = if is_low { "color:var(--danger)" } else { "" };
                            tr class=(low_class) {
                                td class="link-cell mono" style=(danger_style) { (p_code) }
                                td style=(danger_style) { (p_name) }
                                td class="num-right" { (format_decimal(&item.quantity)) }
                                @let avail_style = if is_low { "color:var(--danger);font-weight:600" } else { "" };
                                td class="num-right" style=(avail_style) { (format_decimal(&item.available_qty)) }
                                td {
                                    a style="color:var(--accent);font-size:var(--text-sm);cursor:pointer"
                                        hx-get=(format!("{}?id={}", StockDetailPath::PATH, item.id))
                                        hx-target="#stock-drawer-content"
                                        hx-swap="innerHTML"
                                        hx-on::after-request="hsAdd(null,'#stock-drawer','open')" {
                                        "详情"
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="5" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无库存数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(StockListPath::PATH, &query, result.total, result.page, result.total_pages))
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
        return Ok(Html("<div style=\"padding:var(--space-6);color:var(--muted)\">未找到库存记录</div>".into()));
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
        div class="drawer-section" {
            div class="drawer-label" { "基本信息" }
            div class="detail-info-grid" {
                div class="detail-info-item" { label { "产品编码" } span class="mono" { (p_code) } }
                div class="detail-info-item" { label { "产品名称" } span { (p_name) } }
                div class="detail-info-item" { label { "仓库" } span { (warehouse) } }
                div class="detail-info-item" { label { "库区" } span { (zone) } }
                div class="detail-info-item" { label { "储位" } span class="mono" { (bin) } }
                div class="detail-info-item" { label { "批次号" } span class="mono" { (item.batch_no.as_deref().unwrap_or("—")) } }
            }
        }
        div class="drawer-section" {
            div class="drawer-label" { "库存数量" }
            div class="detail-info-grid" {
                div class="detail-info-item" { label { "现有量" } span class="mono" { (format_decimal(&item.quantity)) } }
                div class="detail-info-item" { label { "已预留" } span class="mono" { (format_decimal(&item.reserved_qty)) } }
                div class="detail-info-item" {
                    label { "可用量" }
                    span class={"mono" (if is_low { " danger" } else { "" })} { (format_decimal(&item.available_qty)) }
                }
                div class="detail-info-item" {
                    label { "安全库存" }
                    span class={"mono" (if is_low { " warn" } else { "" })} { (format_decimal(&safe_stock)) }
                }
            }
            div style="margin-top:12px" {
                div style="display:flex;justify-content:space-between;font-size:var(--text-xs);color:var(--muted);margin-bottom:4px" {
                    span { "可用量 / 安全库存" }
                    span style=(format!("font-weight:600;color:{bar_color}")) { (format!("{pct}%")) }
                }
                div class="stock-bar-wrap" {
                    div class="stock-bar-fill" style=(format!("width:{pct}%;background:{bar_color}")) {}
                }
            }
        }
        div class="drawer-section" {
            div class="drawer-label" { "财务与日期" }
            div class="detail-info-grid" {
                div class="detail-info-item" { label { "单位成本" } span class="mono" { (item.unit_cost.map(|c| format!("¥{}", format_decimal(&c))).unwrap_or_else(|| "—".into())) } }
                div class="detail-info-item" { label { "库存金额" } span class="mono" { (stock_value.map(|v| format!("¥{}", format_decimal(&v))).unwrap_or_else(|| "—".into())) } }
                div class="detail-info-item" { label { "入库日期" } span class="mono" { (item.received_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into())) } }
                div class="detail-info-item" { label { "有效期" } span class="mono" { (item.expiry_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into())) } }
            }
        }
    }
}
