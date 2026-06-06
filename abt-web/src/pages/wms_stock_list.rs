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
use crate::routes::wms_stock::{StockListPath, StockTablePath, StockZonesPath};
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

fn build_filter(params: &StockQueryParams, single_product_id: Option<i64>) -> StockFilter {
    StockFilter {
        product_id: single_product_id,
        warehouse_id: params.warehouse_id,
        zone_id: params.zone_id,
        bin_id: None,
        batch_no: params.batch_no.clone(),
    }
}

fn build_query_string(params: &StockQueryParams) -> String {
    let mut q = vec![];
    if let Some(code) = &params.product_code {
        if !code.is_empty() {
            q.push(format!("product_code={code}"));
        }
    }
    if let Some(name) = &params.product_name {
        if !name.is_empty() {
            q.push(format!("product_name={name}"));
        }
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
    if let Some(bn) = &params.batch_no {
        if !bn.is_empty() {
            q.push(format!("batch_no={bn}"));
        }
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
    let has_code = params.product_code.as_deref().map_or(false, |s| !s.trim().is_empty());
    let has_name = params.product_name.as_deref().map_or(false, |s| !s.trim().is_empty());

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
        if !map.contains_key(&id) {
            if let Ok(wh) = svc.get(ctx, db, id).await {
                map.insert(id, wh.name);
            }
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
        if !map.contains_key(&zid) {
            if let Ok(result) = svc.list_bins(ctx, db, zid, None, 1, 500).await {
                for b in result.items {
                    map.insert(b.id, b.code);
                }
            }
        }
    }
    map
}

fn format_decimal(d: &Decimal) -> String {
    let v: f64 = d.to_f64().unwrap_or(0.0);
    if v == (v as i64) as f64 {
        format!("{}", v as i64)
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

#[require_permission("WMS", "read")]
pub async fn get_stock_list(
    _path: StockListPath,
    ctx: RequestContext,
    Query(params): Query<StockQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.stock_ledger_service();
    let product_svc = state.product_service();
    let warehouse_svc = state.warehouse_service();

    let has_search = params.product_code.as_deref().map_or(false, |s| !s.trim().is_empty())
        || params.product_name.as_deref().map_or(false, |s| !s.trim().is_empty());
    let (single_pid, all_pids) = resolve_product_search(&product_svc, &service_ctx, &mut conn, &params).await.unwrap_or((None, vec![]));
    let filter = build_filter(&params, single_pid);
    let page_num = params.page.unwrap_or(1);
    let mut result = svc.query(&service_ctx, &mut conn, filter, page_num, 20).await?;

    // 搜索了但没找到匹配产品 → 清空结果
    if has_search && all_pids.is_empty() && single_pid.is_none() {
        result.items.clear();
    }
    // 多个产品匹配 → post-filter
    else if !all_pids.is_empty() && all_pids.len() > 1 {
        let id_set: HashSet<i64> = all_pids.into_iter().collect();
        result.items.retain(|item| id_set.contains(&item.product_id));
    }

    let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &result.items).await;
    let warehouse_names = resolve_warehouse_names(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let zone_codes = resolve_zone_codes(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let bin_codes = resolve_bin_codes(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let warehouses = warehouse_svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await.map(|r| r.items).unwrap_or_default();
    let zones = if let Some(wid) = params.warehouse_id {
        warehouse_svc.list_zones(&service_ctx, &mut conn, wid).await.unwrap_or_default()
    } else {
        vec![]
    };
    let content = stock_list_page(&result, &product_names, &warehouse_names, &zone_codes, &bin_codes, &warehouses, &zones, &params);
    let page_html = admin_page(
        is_htmx, "库存查询", &claims, "inventory", StockListPath::PATH, "库存管理", None, content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "read")]
pub async fn get_stock_table(
    _path: StockTablePath,
    ctx: RequestContext,
    Query(params): Query<StockQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.stock_ledger_service();
    let product_svc = state.product_service();
    let warehouse_svc = state.warehouse_service();

    let has_search = params.product_code.as_deref().map_or(false, |s| !s.trim().is_empty())
        || params.product_name.as_deref().map_or(false, |s| !s.trim().is_empty());
    let (single_pid, all_pids) = resolve_product_search(&product_svc, &service_ctx, &mut conn, &params).await.unwrap_or((None, vec![]));
    let filter = build_filter(&params, single_pid);
    let page_num = params.page.unwrap_or(1);
    let mut result = svc.query(&service_ctx, &mut conn, filter, page_num, 20).await?;

    // 搜索了但没找到匹配产品 → 清空结果
    if has_search && all_pids.is_empty() && single_pid.is_none() {
        result.items.clear();
    }
    // 多个产品匹配 → post-filter
    else if !all_pids.is_empty() && all_pids.len() > 1 {
        let id_set: HashSet<i64> = all_pids.into_iter().collect();
        result.items.retain(|item| id_set.contains(&item.product_id));
    }

    let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &result.items).await;
    let warehouse_names = resolve_warehouse_names(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let zone_codes = resolve_zone_codes(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let bin_codes = resolve_bin_codes(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    // HTMX partial: return only the data-card
    Ok(Html(stock_data_card(&result, &product_names, &warehouse_names, &zone_codes, &bin_codes, &params).into_string()))
}

// ── Components ──

fn stock_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::stock_ledger::model::StockLedger>,
    product_names: &HashMap<i64, (String, String)>,
    warehouse_names: &HashMap<i64, String>,
    zone_codes: &HashMap<i64, String>,
    bin_codes: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    zones: &[abt_core::wms::warehouse::model::Zone],
    params: &StockQueryParams,
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
            (stock_filter_bar(warehouses, zones, params))
            // ── Data Card (HTMX target) ──
            (stock_data_card(result, product_names, warehouse_names, zone_codes, bin_codes, params))
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
    warehouse_names: &HashMap<i64, String>,
    zone_codes: &HashMap<i64, String>,
    bin_codes: &HashMap<i64, String>,
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
                            th { "仓库" }
                            th { "库区" }
                            th { "储位" }
                            th { "批次号" }
                            th class="num-right" { "现有量" }
                            th class="num-right" { "已预留量" }
                            th class="num-right" { "可用量" }
                            th class="num-right" { "单位成本" }
                            th class="num-right" { "安全库存" }
                            th { "入库日期" }
                            th { "有效期" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let product_info = product_names.get(&item.product_id);
                            @let p_code = product_info.map(|(c, _)| c.as_str()).unwrap_or("—");
                            @let p_name = product_info.map(|(_, n)| n.as_str()).unwrap_or("—");
                            @let wh_name = warehouse_names.get(&item.warehouse_id).map(|s| s.as_str()).unwrap_or("—");
                            @let z_code = zone_codes.get(&item.zone_id).map(|s| s.as_str()).unwrap_or("—");
                            @let b_code = bin_codes.get(&item.bin_id).map(|s| s.as_str()).unwrap_or("—");
                            @let is_low = item.available_qty <= Decimal::ZERO;
                            @let low_class = if is_low { "text-low-stock" } else { "" };
                            @let danger_style = if is_low { "color:var(--danger)" } else { "" };
                            tr class=(low_class) {
                                td class="link-cell mono" style=(danger_style) { (p_code) }
                                td style=(danger_style) { (p_name) }
                                td { (wh_name) }
                                td { (z_code) }
                                td class="mono" { (b_code) }
                                td class="mono" { (item.batch_no.as_deref().unwrap_or("—")) }
                                td class="num-right" { (format_decimal(&item.quantity)) }
                                td class="num-right" { (format_decimal(&item.reserved_qty)) }
                                @let avail_style = if is_low { "color:var(--danger);font-weight:600" } else { "" };
                                td class="num-right" style=(avail_style) { (format_decimal(&item.available_qty)) }
                                td class="num-right" { (item.unit_cost.map(|c| format!("¥{}", format_decimal(&c))).unwrap_or_else(|| "—".into())) }
                                td class="num-right" { "—" }
                                td class="mono" style="color:var(--muted)" { (item.received_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into())) }
                                td style="color:var(--muted)" { (item.expiry_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into())) }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="13" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
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
