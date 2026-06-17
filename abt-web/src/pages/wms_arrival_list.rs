use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::arrival_notice::model::ArrivalNoticeFilter;
use abt_core::wms::arrival_notice::ArrivalNoticeService;
use abt_core::wms::enums::ArrivalStatus;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_arrival::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ArrivalQueryParams {
    pub doc_number: Option<String>,
    pub supplier: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn build_filter(params: &ArrivalQueryParams) -> ArrivalNoticeFilter {
    ArrivalNoticeFilter {
        doc_number: params.doc_number.clone(),
        status: params.status.and_then(ArrivalStatus::from_i16),
        supplier_id: None,
        warehouse_id: params.warehouse_id,
    }
}

fn build_query_string(params: &ArrivalQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.doc_number {
        q.push(format!("doc_number={v}"));
    }
    if let Some(ref v) = params.supplier {
        q.push(format!("supplier={v}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(wid) = params.warehouse_id {
        q.push(format!("warehouse_id={wid}"));
    }
    q.join("&")
}

fn status_label(s: ArrivalStatus) -> (&'static str, &'static str) {
    match s {
        ArrivalStatus::Draft => ("草稿", "status-draft"),
        ArrivalStatus::Received => ("已收货", "status-received"),
        ArrivalStatus::Inspecting => ("检验中", "status-inspecting"),
        ArrivalStatus::Accepted => ("已接收", "status-completed"),
        ArrivalStatus::PartiallyAccepted => ("部分接收", "status-partial"),
        ArrivalStatus::Rejected => ("已拒收", "status-danger"),
        ArrivalStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

async fn resolve_warehouse_names<S: WarehouseService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    notices: &[abt_core::wms::arrival_notice::model::ArrivalNotice],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = notices.iter().map(|n| n.warehouse_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let mut map = HashMap::new();
    if let Ok(wh_result) = svc.list(ctx, db, WarehouseFilter::default(), 1, 200).await {
        for wh in &wh_result.items {
            if ids.contains(&wh.id) {
                map.insert(wh.id, wh.name.clone());
            }
        }
    }
    map
}

async fn resolve_supplier_names<S: SupplierService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    notices: &[abt_core::wms::arrival_notice::model::ArrivalNotice],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = notices.iter().map(|n| n.supplier_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let mut map = HashMap::new();
    if let Ok(result) = svc.list(ctx, db, SupplierQuery::default(), PageParams::new(1, 500)).await {
        for s in &result.items {
            if ids.contains(&s.id) {
                map.insert(s.id, s.name.clone());
            }
        }
    }
    map
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_arrival_list(
    _path: ArrivalListPath,
    ctx: RequestContext,
    Query(params): Query<ArrivalQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("INVENTORY", "create").await;
    let can_delete = ctx.has_permission("INVENTORY", "delete").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.arrival_notice_service();
    let warehouse_svc = state.warehouse_service();
    let supplier_svc = state.supplier_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;

    let warehouse_names = resolve_warehouse_names(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;

    let content = arrival_list_page(&result, &warehouse_names, &supplier_names, &warehouses.items, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "来料通知", &claims, "inventory", ArrivalListPath::PATH, "库存管理", Some("来料通知"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn arrival_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<abt_core::wms::arrival_notice::model::ArrivalNotice>,
    warehouse_names: &HashMap<i64, String>,
    supplier_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &ArrivalQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "来料通知" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ArrivalCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建来料通知"
                        }
                    }
                }
            }
            (arrival_table_fragment(result, warehouse_names, supplier_names, warehouses, params, can_delete))
        }
    }
}

fn arrival_table_fragment(
    result: &abt_core::shared::types::pagination::PaginatedResult<abt_core::wms::arrival_notice::model::ArrivalNotice>,
    warehouse_names: &HashMap<i64, String>,
    supplier_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &ArrivalQueryParams,
    can_delete: bool,
) -> Markup {
    let _query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已收货", count: None },
        TabItem { value: "3".into(), label: "检验中", count: None },
        TabItem { value: "4".into(), label: "已接收", count: None },
        TabItem { value: "5".into(), label: "部分接收", count: None },
        TabItem { value: "6".into(), label: "已拒收", count: None },
        TabItem { value: "7".into(), label: "已取消", count: None },
    ];

    let selected_warehouse = params.warehouse_id.map(|id| id.to_string()).unwrap_or_default();

    html! {
        div class="arrival-list-panel" {
            (status_tabs_with_param(ArrivalListPath::PATH, "#arrival-data-card", "#arrival-filter-form", tabs, &active_value, "status"))

            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="arrival-filter-form"
                hx-get=(ArrivalListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#arrival-data-card"
                hx-select="#arrival-data-card"
                hx-swap="outerHTML"
                hx-include="#arrival-filter-form"
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
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="supplier"
                        placeholder="供应商"
                        value=(params.supplier.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="warehouse_id" {
                    option value="" { "全部仓库" }
                    @for w in warehouses {
                        option value=(w.id) selected[selected_warehouse == w.id.to_string()] { (w.name) }
                    }
                }
            }

            (arrival_data_card(result, warehouse_names, supplier_names, params, can_delete))
        }
    }
}

fn arrival_data_card(
    result: &abt_core::shared::types::pagination::PaginatedResult<abt_core::wms::arrival_notice::model::ArrivalNotice>,
    warehouse_names: &HashMap<i64, String>,
    supplier_names: &HashMap<i64, String>,
    params: &ArrivalQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="arrival-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100" {
                    thead {
                        tr {
                            th { "单据编号" }
                            th { "来源采购单" }
                            th { "供应商" }
                            th { "到货仓库" }
                            th { "到货日期" }
                            th { "状态" }
                            th class="!text-right" { "操作" }
                        }
                    }
                    tbody {
                        @for n in &result.items {
                            (arrival_row(n, warehouse_names, supplier_names, can_delete))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无来料通知数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(ArrivalListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}

fn arrival_row(
    n: &abt_core::wms::arrival_notice::model::ArrivalNotice,
    warehouse_names: &HashMap<i64, String>,
    supplier_names: &HashMap<i64, String>,
    can_delete: bool,
) -> Markup {
    let detail_path = ArrivalDetailPath { id: n.id };
    let onclick = format!("location.href='{}'", detail_path);
    let (status_text, status_class) = status_label(n.status);
    let warehouse_name = warehouse_names.get(&n.warehouse_id).map(|s| s.as_str()).unwrap_or("—");
    let supplier_name = supplier_names.get(&n.supplier_id).map(|s| s.as_str()).unwrap_or("—");
    let is_draft = n.status == ArrivalStatus::Draft;

    html! {
        tr style="cursor:pointer" {
            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) { (n.doc_number) }
            td class="font-mono tabular-nums" onclick=(&onclick) { "—" }
            td onclick=(&onclick) { (supplier_name) }
            td onclick=(&onclick) { (warehouse_name) }
            td class="font-mono tabular-nums" onclick=(&onclick) { (n.arrival_date.format("%Y-%m-%d")) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                        a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(ArrivalCreatePath::PATH) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除" {
                                (icon::trash_icon("w-4 h-4"))
                            }
                        }
                    }
                } @else {
                    div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                        a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(detail_path.to_string()) title="查看" {
                            (icon::eye_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除" {
                                (icon::trash_icon("w-4 h-4"))
                            }
                        }
                    }
                }
            }
        }
    }
}
