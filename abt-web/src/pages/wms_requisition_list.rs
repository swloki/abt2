use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::UserService;
use abt_core::wms::enums::RequisitionStatus;
use abt_core::wms::material_requisition::model::RequisitionFilter;
use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::{RequisitionCreatePath, RequisitionDetailPath, RequisitionListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RequisitionQueryParams {
    pub doc_number: Option<String>,
    pub work_order: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn build_filter(params: &RequisitionQueryParams) -> RequisitionFilter {
    RequisitionFilter {
        doc_number: params.doc_number.clone(),
        status: params.status.and_then(RequisitionStatus::from_i16),
        work_order_id: None,
        warehouse_id: params.warehouse_id,
    }
}

fn build_query_string(params: &RequisitionQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.doc_number {
        q.push(format!("doc_number={v}"));
    }
    if let Some(ref v) = params.work_order {
        q.push(format!("work_order={v}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(wid) = params.warehouse_id {
        q.push(format!("warehouse_id={wid}"));
    }
    q.join("&")
}

fn status_label(s: RequisitionStatus) -> (&'static str, &'static str) {
    match s {
        RequisitionStatus::Draft => ("草稿", "status-draft"),
        RequisitionStatus::Confirmed => ("已确认", "status-confirmed"),
        RequisitionStatus::Issued => ("已发料", "status-completed"),
        RequisitionStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

async fn resolve_warehouse_names<S: WarehouseService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    requisitions: &[abt_core::wms::material_requisition::model::MaterialRequisition],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = requisitions.iter().map(|r| r.warehouse_id).collect();
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

async fn resolve_operator_names<S: UserService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    requisitions: &[abt_core::wms::material_requisition::model::MaterialRequisition],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = requisitions.iter().map(|r| r.operator_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let mut map = HashMap::new();
    for &id in &ids {
        if map.contains_key(&id) {
            continue;
        }
        if let Ok(user) = svc.get_user(ctx, db, id).await {
            map.insert(id, user.display_name.unwrap_or(user.username));
        }
    }
    map
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_requisition_list(
    _path: RequisitionListPath,
    ctx: RequestContext,
    Query(params): Query<RequisitionQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("INVENTORY", "create").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.material_requisition_service();
    let warehouse_svc = state.warehouse_service();
    let user_svc = state.user_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;

    let warehouse_names = resolve_warehouse_names(&warehouse_svc, &service_ctx, &mut conn, &result.items).await;
    let operator_names = resolve_operator_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;

    let content = requisition_list_page(&result, &warehouse_names, &operator_names, &warehouses.items, &params, can_create);
    let page_html = admin_page(
        is_htmx, "领料单", &claims, "inventory", RequisitionListPath::PATH, "库存管理", Some("领料单"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn requisition_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<abt_core::wms::material_requisition::model::MaterialRequisition>,
    warehouse_names: &HashMap<i64, String>,
    operator_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &RequisitionQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "领料单" }
                div class="page-actions" {
                    @if can_create {
                        a class="btn btn-primary" href=(RequisitionCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建领料单"
                        }
                    }
                }
            }
            (requisition_table_fragment(result, warehouse_names, operator_names, warehouses, params))
        }
    }
}

fn requisition_table_fragment(
    result: &abt_core::shared::types::pagination::PaginatedResult<abt_core::wms::material_requisition::model::MaterialRequisition>,
    warehouse_names: &HashMap<i64, String>,
    operator_names: &HashMap<i64, String>,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    params: &RequisitionQueryParams,
) -> Markup {
    let _query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "已发料", count: None },
        TabItem { value: "4".into(), label: "已取消", count: None },
    ];

    let selected_warehouse = params.warehouse_id.map(|id| id.to_string()).unwrap_or_default();

    html! {
        div class="requisition-list-panel" {
            (status_tabs_with_param(RequisitionListPath::PATH, "#requisition-data-card", "#requisition-filter-form", tabs, &active_value, "status"))

            form class="filter-bar filter-form" id="requisition-filter-form"
                hx-get=(RequisitionListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#requisition-data-card"
                hx-select="#requisition-data-card"
                hx-swap="outerHTML"
                hx-include="#requisition-filter-form"
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
                    input class="search-input" type="text" name="work_order"
                        placeholder="关联工单"
                        value=(params.work_order.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="warehouse_id" {
                    option value="" { "全部仓库" }
                    @for w in warehouses {
                        option value=(w.id) selected[selected_warehouse == w.id.to_string()] { (w.name) }
                    }
                }
            }

            (requisition_data_card(result, warehouse_names, operator_names, params))
        }
    }
}

fn requisition_data_card(
    result: &abt_core::shared::types::pagination::PaginatedResult<abt_core::wms::material_requisition::model::MaterialRequisition>,
    warehouse_names: &HashMap<i64, String>,
    operator_names: &HashMap<i64, String>,
    params: &RequisitionQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="data-card" id="requisition-data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "单据编号" }
                            th { "关联工单" }
                            th { "领料仓库" }
                            th { "领料日期" }
                            th { "状态" }
                            th { "操作员" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for r in &result.items {
                            (requisition_row(r, warehouse_names, operator_names))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无领料单数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(RequisitionListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}

fn requisition_row(
    r: &abt_core::wms::material_requisition::model::MaterialRequisition,
    warehouse_names: &HashMap<i64, String>,
    operator_names: &HashMap<i64, String>,
) -> Markup {
    let detail_path = RequisitionDetailPath { id: r.id };
    let onclick = format!("location.href='{}'", detail_path);
    let (status_text, status_class) = status_label(r.status);
    let warehouse_name = warehouse_names.get(&r.warehouse_id).map(|s| s.as_str()).unwrap_or("—");
    let operator_name = operator_names.get(&r.operator_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(&onclick) { (r.doc_number) }
            td class="mono" onclick=(&onclick) { "WO-" (r.work_order_id) }
            td onclick=(&onclick) { (warehouse_name) }
            td class="mono" onclick=(&onclick) { (r.requisition_date.format("%Y-%m-%d")) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick=(&onclick) { (operator_name) }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" href=(detail_path.to_string()) title="查看" {
                        (icon::eye_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}
