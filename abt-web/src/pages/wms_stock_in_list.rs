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
use crate::routes::wms_stock_in::{StockInCreatePath, StockInListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct StockInQueryParams {
    pub doc_number: Option<String>,
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

fn transaction_type_label(t: &TransactionType) -> (&'static str, &'static str, &'static str) {
    // (label, bg_class, text_class)
    match t {
        TransactionType::PurchaseReceipt => ("采购入库", "rgba(22,119,255,0.08)", "var(--accent)"),
        TransactionType::ProductionReceipt => ("生产入库", "rgba(82,196,26,0.08)", "var(--success)"),
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

fn build_query_string(params: &StockInQueryParams) -> String {
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
pub async fn get_stock_in_list(
    _path: StockInListPath,
    ctx: RequestContext,
    Query(params): Query<StockInQueryParams>,
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

    let content = stock_in_list_page(&result, &operator_names, &wh_names, &warehouses, &params, can_create);
    let page_html = admin_page(
        is_htmx, "入库管理", &claims, "inventory", StockInListPath::PATH, "库存管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn stock_in_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
    operator_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &StockInQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "入库管理" }
                div class="flex gap-3" {
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" {
                        (icon::download_icon("w-4 h-4"))
                        "导出"
                    }
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(StockInCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建入库单"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Table (HTMX panel) ──
            (stock_in_table_fragment(result, operator_names, wh_names, warehouses, params))
        }
    }
}

fn stock_in_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
    operator_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &StockInQueryParams,
) -> Markup {
    let _query = build_query_string(params);
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "PurchaseReceipt".into(), label: "采购入库", count: None },
        TabItem { value: "ProductionReceipt".into(), label: "生产入库", count: None },
    ];

    let selected_type = params.transaction_type.as_deref().unwrap_or("");

    html! {
        div class="stockin-list-panel" {
            // ── Stat Cards ──
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-5);margin-bottom:var(--space-6)" {
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" {
                        (icon::download_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (total_count) }
                        div class="text-sm text-text-muted mt-1" { "本月入库单" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 green" {
                        (icon::currency_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { "—" }
                        div class="text-sm text-text-muted mt-1" { "入库总金额" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 orange" {
                        (icon::clock_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { "—" }
                        div class="text-sm text-text-muted mt-1" { "待审核" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:linear-gradient(135deg,#f0e6ff,#e0d0ff);color:#7c3aed" {
                        (icon::check_circle_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (total_count) }
                        div class="text-sm text-text-muted mt-1" { "已完成" }
                    }
                }
            }

            (status_tabs_with_param(StockInListPath::PATH, "#stock-in-data-card", "#stock-in-filter-form", tabs, selected_type, "transaction_type"))

            // ── Filter Bar ──
            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="stock-in-filter-form"
                hx-get=(StockInListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#stock-in-data-card"
                hx-select="#stock-in-data-card"
                hx-swap="outerHTML"
                hx-include="#stock-in-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="doc_number"
                        style="width:180px"
                        placeholder="单据编号"
                        value=(params.doc_number.as_deref().unwrap_or(""));
                }
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="product_code"
                        placeholder="物料编码"
                        value=(params.product_code.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="transaction_type" {
                    option value="" selected[selected_type.is_empty()] { "入库类型" }
                    option value="PurchaseReceipt" selected[selected_type == "PurchaseReceipt"] { "采购入库" }
                    option value="ProductionReceipt" selected[selected_type == "ProductionReceipt"] { "生产入库" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="warehouse_id" {
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
            (stock_in_data_card(result, operator_names, wh_names, warehouses, params))

            // ── Info Note ──
            div style="margin-top:var(--space-6);padding:var(--space-4) var(--space-5);background:var(--accent-bg);border:1px solid rgba(22,119,255,0.15);border-radius:var(--radius-md);display:flex;align-items:flex-start;gap:var(--space-3)" {
                (icon::circle_alert_icon("w-4 h-4"))
                div style="font-size:var(--text-sm);color:var(--fg-2);line-height:1.6" {
                    strong { "入库流程说明：" }
                    "入库操作通过 InventoryTransactionService.record() 执行，每次入库自动生成 InventoryTransaction 记录并更新 StockLedger 库存账，单据号格式为 RK-YYYY-MM-SEQ（如 RK-2026-06-000001）。"
                    "采购入库需关联来料通知单（IQC质检通过后）；生产入库关联工单完工报工。"
                }
            }
        }
    }
}

fn stock_in_data_card(
    result: &abt_core::shared::types::PaginatedResult<abt_core::wms::inventory_transaction::model::InventoryTransaction>,
    operator_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    _warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &StockInQueryParams,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="stock-in-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th style="width:30px" { input type="checkbox"; }
                            th { "入库单号" }
                            th { "入库类型" }
                            th { "来源单号" }
                            th { "目标仓库" }
                            th { "物料数量" }
                            th class="text-right text-[13px]" { "入库总量" }
                            th class="text-right text-[13px]" { "总金额" }
                            th { "状态" }
                            th { "操作员" }
                            th { "入库时间" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (type_label, type_bg, type_color) = transaction_type_label(&item.transaction_type);
                            @let wh_name = wh_names.get(&item.warehouse_id).map(|s| s.as_str()).unwrap_or("—");
                            @let op_name = operator_names.get(&item.operator_id).map(|s| s.as_str()).unwrap_or("—");
                            tr {
                                td { input type="checkbox"; }
                                td class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="color:var(--accent)" { (item.doc_number.as_deref().unwrap_or("—")) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
                                        (type_label)
                                    }
                                }
                                td class="font-mono tabular-nums" style="color:var(--fg-2);font-size:12px" {
                                    @if let Some(ref sn) = item.source_doc_number {
                                        (sn)
                                    } @else {
                                        span style="color:var(--muted)" { "—" }
                                    }
                                }
                                td { (wh_name) }
                                td { "1 种" }
                                td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.quantity)) }
                                td class="text-right text-[13px] font-mono tabular-nums" { (item.unit_cost.map(|c| format!("¥{:.2}", c)).unwrap_or_else(|| "—".into())) }
                                td {
                                    span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#f0fff0] text-[#389e0d]" { "已入库" }
                                }
                                td { (op_name) }
                                td style="font-size:12px;color:var(--muted)" { (item.created_at.format("%Y-%m-%d %H:%M")) }
                                td {
                                    a href=(format!("/admin/wms/stock-in/{}", item.id)) style="color:var(--accent);font-size:var(--text-xs)" { "详情" }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="12" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无入库记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(StockInListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
