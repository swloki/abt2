use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::model::*;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::{PageParams, PgExecutor, ServiceContext};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::OrderDetailPath;
use crate::routes::shipping::*;
use crate::utils::{empty_as_none, resolve_customer_names, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ShippingQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub customer_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn build_query_string(params: &ShippingQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(c) = params.customer_id {
        q.push(format!("customer_id={c}"));
    }
    q.join("&")
}

fn status_label(s: ShippingStatus) -> (&'static str, &'static str) {
    match s {
        ShippingStatus::Draft => ("待审核", "status-draft"),
        ShippingStatus::Confirmed => ("已确认", "status-confirmed"),
        ShippingStatus::Picking => ("拣货中", "status-picking"),
        ShippingStatus::Shipped => ("已发货", "status-shipped"),
        ShippingStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

/// Compute status counts by calling ShippingRequestService::list for each status with page_size=1.
async fn count_by_status<S: ShippingRequestService>(
    svc: &S,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    customer_id: Option<i64>,
) -> HashMap<i16, u64> {
    let statuses = [
        (ShippingStatus::Draft, 1i16),
        (ShippingStatus::Confirmed, 2),
        (ShippingStatus::Picking, 3),
        (ShippingStatus::Shipped, 4),
        (ShippingStatus::Cancelled, 5),
    ];

    let mut counts = HashMap::new();
    for (status, code) in statuses {
        let filter = ShippingQuery {
            order_id: None,
            status: Some(status),
            keyword: None,
            customer_id,
        };
        let page = PageParams::new(1, 1);
        if let Ok(result) = svc.list(ctx, db, filter, page).await {
            counts.insert(code, result.total);
        }
    }

    let total: u64 = counts.values().sum();
    counts.insert(0, total);

    counts
}

/// Resolve order doc_numbers by calling SalesOrderService::find_by_id for each unique id.
async fn resolve_order_numbers<S: SalesOrderService>(
    svc: &S,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    items: &[ShippingRequest],
) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    let mut seen = HashSet::new();
    for item in items {
        if let Some(oid) = item.order_id {
            if seen.insert(oid)
                && let Ok(order) = svc.find_by_id(ctx, db, oid).await {
                    map.insert(oid, order.doc_number);
                }
        }
    }
    map
}

// ── Handlers ──

#[require_permission("SHIPPING", "read")]
pub async fn get_shipping_list(
    _path: ShippingListPath,
    ctx: RequestContext,
    Query(params): Query<ShippingQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("SHIPPING", "create").await;
    let can_delete = ctx.has_permission("SHIPPING", "delete").await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let shipping_svc = state.shipping_service();
    let customer_svc = state.customer_service();
    let order_svc = state.sales_order_service();
    let filter = ShippingQuery {
        order_id: None,
        status: params.status.and_then(ShippingStatus::from_i16),
        keyword: params.keyword.clone(),
        customer_id: params.customer_id,
    };
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = shipping_svc.list(&service_ctx, &mut conn, filter, page).await?;

    let status_counts = count_by_status(&shipping_svc, &service_ctx, &mut conn, params.customer_id).await;
    let customer_names = resolve_customer_names(&customer_svc, &service_ctx, &mut conn, result.items.iter().map(|i| i.customer_id)).await;
    let order_numbers = resolve_order_numbers(&order_svc, &service_ctx, &mut conn, &result.items).await;

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let content = shipping_list_page(&claims, &result, &customer_names, &order_numbers, &customers.items, &params, &status_counts, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "发货申请", &claims, "sales", ShippingListPath::PATH, "销售管理", Some("发货申请"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SHIPPING", "delete")]
pub async fn delete_shipping(
    path: ShippingDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let shipping_svc = state.shipping_service();
    shipping_svc.delete(&service_ctx, &mut conn, path.id).await?;

    let redirect = ShippingListPath::PATH.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn shipping_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<ShippingRequest>,
    customer_names: &HashMap<i64, String>,
    order_numbers: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ShippingQueryParams,
    status_counts: &HashMap<i16, u64>,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "发货申请" }
                div class="page-actions" {
                    @if can_create {
                        a class="btn btn-primary" href=(ShippingCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建发货申请"
                        }
                    }
                }
            }
            (shipping_table_fragment(result, customer_names, order_numbers, customers, params, status_counts, can_delete))
        }
    }
}

fn shipping_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<ShippingRequest>,
    customer_names: &HashMap<i64, String>,
    order_numbers: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ShippingQueryParams,
    status_counts: &HashMap<i16, u64>,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();

    let total_count: u64 = status_counts.values().sum();
    let draft_count = status_counts.get(&1).copied();
    let confirmed_count = status_counts.get(&2).copied();
    let picking_count = status_counts.get(&3).copied();
    let shipped_count = status_counts.get(&4).copied();
    let cancelled_count = status_counts.get(&5).copied();

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "待审核", count: draft_count },
        TabItem { value: "2".into(), label: "已确认", count: confirmed_count },
        TabItem { value: "3".into(), label: "拣货中", count: picking_count },
        TabItem { value: "4".into(), label: "已发货", count: shipped_count },
        TabItem { value: "5".into(), label: "已取消", count: cancelled_count },
    ];

    let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();

    html! {
        div class="shipping-list-panel" {
            (status_tabs_with_param(ShippingListPath::PATH, "#shipping-data-card", "#shipping-filter-form", tabs, &active_value, "status"))

            form class="filter-bar filter-form" id="shipping-filter-form"
                hx-get=(ShippingListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#shipping-data-card"
                hx-select="#shipping-data-card"
                hx-swap="outerHTML"
                hx-include="#shipping-filter-form"
                hx-push-url="true" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索发货单号、客户名称…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="customer_id" {
                    option value="" { "全部客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
                    }
                }
            }

            div class="data-card" id="shipping-data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "发货单号" }
                                th { "来源订单" }
                                th { "客户名称" }
                                th { "状态" }
                                th { "预计发货日期" }
                                th { "承运商" }
                                th { "物流单号" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for s in &result.items {
                                (shipping_row(s, customer_names, order_numbers, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="9" class="td-empty" {
                                        "暂无发货数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(ShippingListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn shipping_row(
    s: &ShippingRequest,
    customer_names: &HashMap<i64, String>,
    order_numbers: &HashMap<i64, String>,
    can_delete: bool,
) -> Markup {
    let detail_path = ShippingDetailPath { id: s.id };
    let (status_text, status_class) = status_label(s.status);
    let customer_name = customer_names.get(&s.customer_id).map(|n| n.as_str()).unwrap_or("—");
    let order_num = s.order_id.and_then(|oid| order_numbers.get(&oid).map(|n| n.as_str())).unwrap_or("—");
    let ship_date = s.expected_ship_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
    let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = s.status == ShippingStatus::Draft;
    let delete_path = ShippingDeletePath { id: s.id };
    let order_detail_path = s.order_id.map(|oid| OrderDetailPath { id: oid });

    html! {
        tr {
            td class="link-cell mono" onclick=(&onclick) { (s.doc_number) }
            td onclick=(&onclick) {
                @if let Some(odp) = order_detail_path {
                    a href=(odp.to_string()) class="text-accent" onclick="event.stopPropagation()" { (order_num) }
                } @else {
                    (order_num)
                }
            }
            td onclick=(&onclick) { (customer_name) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick=(&onclick) { (ship_date) }
            td onclick=(&onclick) { (s.carrier.as_str()) }
            td class="mono" onclick=(&onclick) { (s.tracking_number.as_str()) }
            td onclick=(&onclick) { (created) }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    @if is_draft {
                        a class="row-action-btn" href=(ShippingEditPath { id: s.id }.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="row-action-btn text-danger" title="删除"
                                hx-confirm=(format!("确认删除发货申请 {}？", s.doc_number))
                                hx-post=(delete_path.to_string())
                                hx-target="closest tr"
                                hx-swap="outerHTML swap:0.5s" {
                                (icon::trash_icon("w-4 h-4"))
                            }
                        }
                    } @else {
                        a class="row-action-btn" href=(detail_path.to_string()) title="查看详情" {
                            (icon::eye_icon("w-4 h-4"))
                        }
                    }
                }
            }
        }
    }
}
