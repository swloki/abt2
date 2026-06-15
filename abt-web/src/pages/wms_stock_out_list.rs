use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::UserService;
use abt_core::wms::enums::TransactionType;
use abt_core::wms::inventory_transaction::model::TransactionFilter;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::warehouse::{WarehouseFilter, WarehouseService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::{StockOutCreatePath, StockOutListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct StockOutQueryParams {
    pub doc_number: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub product_code: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub transaction_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_start: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_end: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn out_type_label(t: &TransactionType) -> (&'static str, &'static str, &'static str) {
    match t {
        TransactionType::SalesShipment => ("销售出库", "rgba(255,77,79,0.08)", "var(--danger)"),
        TransactionType::MaterialIssue => ("生产领料", "rgba(250,173,20,0.08)", "var(--warn)"),
        _ => ("其他", "rgba(0,0,0,0.04)", "var(--muted)"),
    }
}

async fn resolve_operator_names<S: UserService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
) -> HashMap<i64, String> {
    let unique_ids: Vec<i64> = items.iter().map(|t| t.operator_id).collect();
    if unique_ids.is_empty() {
        return HashMap::new();
    }
    let mut map = HashMap::new();
    for id in unique_ids {
        if !map.contains_key(&id)
            && let Ok(user) = svc.get_user(ctx, db, id).await {
                map.insert(id, user.display_name.unwrap_or_default());
            }
    }
    map
}

async fn resolve_wh_names<S: WarehouseService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[abt_core::wms::inventory_transaction::model::InventoryTransaction],
) -> HashMap<i64, String> {
    let unique_ids: Vec<i64> = items.iter().map(|t| t.warehouse_id).collect();
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

fn build_query_string(params: &StockOutQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.doc_number {
        q.push(format!("doc_number={v}"));
    }
    if let Some(ref v) = params.product_code {
        q.push(format!("product_code={v}"));
    }
    if let Some(ref tt) = params.transaction_type {
        q.push(format!("transaction_type={tt}"));
    }
    if let Some(wid) = params.warehouse_id {
        q.push(format!("warehouse_id={wid}"));
    }
    if let Some(ref ds) = params.date_start {
        q.push(format!("date_start={ds}"));
    }
    if let Some(ref de) = params.date_end {
        q.push(format!("date_end={de}"));
    }
    q.join("&")
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_stock_out_list(
    _path: StockOutListPath,
    ctx: RequestContext,
    Query(params): Query<StockOutQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("INVENTORY", "create").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inventory_transaction_service();
    let user_svc = state.user_service();
    let warehouse_svc = state.warehouse_service();

    let txn_type = params.transaction_type.as_deref().and_then(TransactionType::from_name);
    let filter = TransactionFilter {
        transaction_type: txn_type,
        product_id: None,
        warehouse_id: params.warehouse_id,
        source_type: None,
        source_id: None,
        doc_number: params.doc_number.clone(),
        product_code: params.product_code.clone(),
    };
    let page_num = params.page.unwrap_or(1);
    let result = svc.query(&service_ctx, &mut conn, filter, page_num, 20).await?;

    let operator_names = resolve_operator_names(&user_svc, &service_ctx, &mut conn, &result.items).await;
    let wh_names = resolve_wh_names(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let warehouses = warehouse_svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200).await.map(|r| r.items).unwrap_or_default();

    let content = stock_out_list_page(&result, &operator_names, &wh_names, &warehouses, &params, can_create);
    let page_html = admin_page(
        is_htmx, "出库管理", &claims, "inventory", StockOutListPath::PATH, "库存管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn stock_out_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
    operator_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &StockOutQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "出库管理" }
                div class="page-actions" {
                    button class="btn btn-default" {
                        (icon::download_icon("w-4 h-4"))
                        "导出"
                    }
                    @if can_create {
                        a class="btn btn-primary" href=(StockOutCreatePath::PATH) style="background:var(--danger);border-color:var(--danger)" {
                            (icon::upload_icon("w-4 h-4"))
                            "新建出库单"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Table (HTMX panel) ──
            (stock_out_table_fragment(result, operator_names, wh_names, warehouses, params))
        }
    }
}

fn stock_out_data_card(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
    operator_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    query: &str,
) -> Markup {
    html! {
        div class="data-card" id="stockout-data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th style="width:30px" { input type="checkbox"; }
                            th { "出库单号" }
                            th { "出库类型" }
                            th { "来源单号" }
                            th { "来源仓库" }
                            th { "物料数量" }
                            th class="num-right" { "出库总量" }
                            th class="num-right" { "总金额" }
                            th { "拣货策略" }
                            th { "状态" }
                            th { "操作员" }
                            th { "出库时间" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (type_label, type_bg, type_color) = out_type_label(&item.transaction_type);
                            @let wh_name = wh_names.get(&item.warehouse_id).map(|s| s.as_str()).unwrap_or("—");
                            @let op_name = operator_names.get(&item.operator_id).map(|s| s.as_str()).unwrap_or("—");
                            tr {
                                td { input type="checkbox"; }
                                td class="link-cell mono" style="color:var(--accent)" { (item.doc_number.as_deref().unwrap_or("—")) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
                                        (type_label)
                                    }
                                }
                                td class="mono" style="color:var(--fg-2);font-size:12px" { (format!("{}-{}", item.source_type, item.source_id)) }
                                td { (wh_name) }
                                td { "1 种" }
                                td class="num-right mono" { (format!("{:.2}", item.quantity)) }
                                td class="num-right mono" { (item.unit_cost.map(|c| format!("¥{:.2}", c)).unwrap_or_else(|| "—".into())) }
                                td style="font-size:12px;color:var(--fg-2)" { "FIFO" }
                                td {
                                    span class="status-pill status-completed" { "已出库" }
                                }
                                td { (op_name) }
                                td style="font-size:12px;color:var(--muted)" { (item.created_at.format("%Y-%m-%d %H:%M")) }
                                td {
                                    @let detail_url = format!("{}/{}", StockOutListPath::PATH, item.id);
                                    a href=(detail_url) style="color:var(--accent);font-size:var(--text-xs)" { "详情" }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="13" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无出库记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(StockOutListPath::PATH, query, result.total, result.page, result.total_pages))
        }
    }
}

fn stock_out_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
    operator_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &StockOutQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "SalesShipment".into(), label: "销售出库", count: None },
        TabItem { value: "MaterialIssue".into(), label: "生产领料", count: None },
    ];

    let selected_type = params.transaction_type.as_deref().unwrap_or("");

    html! {
        div class="stockout-list-panel" {
            // ── Stat Cards ──
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-5);margin-bottom:var(--space-6)" {
                div class="stat-card" {
                    div class="stat-icon" style="background:linear-gradient(135deg,#fff1f0,#ffccc7);color:#cf1322" {
                        (icon::upload_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { (total_count) }
                        div class="stat-label" { "本月出库单" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon" style="background:linear-gradient(135deg,#fff1f0,#ffccc7);color:#cf1322" {
                        (icon::currency_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "出库总金额" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        (icon::clock_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "待拣货" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon green" {
                        (icon::check_circle_icon("w-5 h-5"))
                    }
                    div {
                        div class="stat-value" { (total_count) }
                        div class="stat-label" { "已完成" }
                    }
                }
            }

            (status_tabs_with_param(StockOutListPath::PATH, "#stockout-data-card", "#stockout-filter-form", tabs, selected_type, "transaction_type"))

            // ── Filter Bar ──
            form class="filter-bar filter-form" id="stockout-filter-form"
                hx-get=(StockOutListPath::PATH)
                hx-trigger="change,keyup changed delay:300ms from:.search-input"
                hx-target="#stockout-data-card"
                hx-select="#stockout-data-card"
                hx-swap="outerHTML"
                hx-include="#stockout-filter-form"
                hx-push-url="true" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="doc_number"
                        style="width:180px"
                        placeholder="单据编号"
                        value=(params.doc_number.as_deref().unwrap_or(""));
                }
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="product_code"
                        placeholder="物料编码"
                        value=(params.product_code.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="warehouse_id" {
                    option value="" selected[params.warehouse_id.is_none()] { "全部仓库" }
                    @for wh in warehouses {
                        option value=(wh.id) selected[params.warehouse_id == Some(wh.id)] { (wh.name) }
                    }
                }
                input class="filter-input" type="date" name="date_start"
                    style="width:140px"
                    value=(params.date_start.as_deref().unwrap_or(""));
                span style="color:var(--muted);line-height:36px" { "~" }
                input class="filter-input" type="date" name="date_end"
                    style="width:140px"
                    value=(params.date_end.as_deref().unwrap_or(""));
            }

            // ── Data Table ──
            (stock_out_data_card(result, operator_names, wh_names, &query))

            // ── Info Note ──
            div style="margin-top:var(--space-6);padding:var(--space-4) var(--space-5);background:var(--danger-bg);border:1px solid rgba(255,77,79,0.15);border-radius:var(--radius-md);display:flex;align-items:flex-start;gap:var(--space-3)" {
                (icon::circle_alert_icon("w-4 h-4"))
                div style="font-size:var(--text-sm);color:var(--fg-2);line-height:1.6" {
                    strong { "出库流程说明：" }
                    "出库操作通过 InventoryTransactionService.record() 执行，单据号格式为 CK-YYYY-MM-SEQ（如 CK-2026-06-000001）。"
                    "销售出库消耗 SOFT预留，生产领料消耗 HARD预留。"
                    "出库前系统根据拣货策略（FIFO/FEFO/最短路径/整托优先）自动分配拣货路径，出库后扣减库存并记录事务日志。"
                }
            }
        }
    }
}
