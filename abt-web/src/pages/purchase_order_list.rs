use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::identity::UserService;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::purchase::order::model::*;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct POQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_range: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let today = chrono::Local::now().date_naive();
    match range {
        "7d" => (Some(today - chrono::Days::new(7)), None),
        "30d" => (Some(today - chrono::Days::new(30)), None),
        "3m" => (Some(today - chrono::Months::new(3)), None),
        _ => (None, None),
    }
}

fn build_filter(params: &POQueryParams) -> PurchaseOrderQuery {
    let (order_date_start, order_date_end) = params
        .date_range
        .as_deref()
        .map(parse_date_range)
        .unwrap_or((None, None));
    PurchaseOrderQuery {
        supplier_id: params.supplier_id,
        status: params.status.and_then(PurchaseOrderStatus::from_i16),
        order_date_start,
        order_date_end,
    }
}

fn build_query_string(params: &POQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(sid) = params.supplier_id {
        q.push(format!("supplier_id={sid}"));
    }
    if let Some(ref dr) = params.date_range {
        q.push(format!("date_range={dr}"));
    }
    q.join("&")
}

async fn resolve_supplier_names<S: SupplierService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    orders: &[PurchaseOrder],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = orders.iter().map(|o| o.supplier_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let all = svc
        .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 200))
        .await;
    match all {
        Ok(result) => result
            .items
            .into_iter()
            .filter(|s| ids.contains(&s.id))
            .map(|s| (s.id, s.name))
            .collect(),
        Err(_) => HashMap::new(),
    }
}

async fn resolve_buyer_names<S: UserService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    orders: &[PurchaseOrder],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = orders.iter().map(|o| o.operator_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    svc.get_users_by_ids(ctx, db, ids)
        .await
        .map(|users| {
            users.into_iter()
                .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
                .collect()
        })
        .unwrap_or_default()
}

// ── Status Labels ──

fn status_label(s: PurchaseOrderStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseOrderStatus::Draft => ("草稿", "status-draft"),
        PurchaseOrderStatus::Confirmed => ("已确认", "status-confirmed"),
        PurchaseOrderStatus::PartiallyReceived => ("部分收货", "status-partial"),
        PurchaseOrderStatus::Received => ("已收货", "status-success"),
        PurchaseOrderStatus::Closed => ("已关闭", "status-cancelled"),
        PurchaseOrderStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_list(
    _path: POListPath,
    ctx: RequestContext,
    Query(params): Query<POQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("PURCHASE_ORDER", "create").await;
    let can_delete = ctx.has_permission("PURCHASE_ORDER", "delete").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();
    let supplier_svc = state.supplier_service();
    let user_svc = state.user_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
    let buyer_names = resolve_buyer_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    let content = po_list_page(&result, &supplier_names, &buyer_names, &suppliers.items, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "采购订单", &claims, "purchase", POListPath::PATH, "采购管理", Some("采购订单"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_table(
    ctx: RequestContext,
    Query(params): Query<POQueryParams>,
) -> Result<Html<String>> {
    let can_delete = ctx.has_permission("PURCHASE_ORDER", "delete").await;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();
    let supplier_svc = state.supplier_service();
    let user_svc = state.user_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
    let buyer_names = resolve_buyer_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    Ok(Html(po_table_fragment(&result, &supplier_names, &buyer_names, &suppliers.items, &params, can_delete).into_string()))
}

// ── Components ──

fn po_list_page(
    result: &abt_core::shared::types::PaginatedResult<PurchaseOrder>,
    supplier_names: &HashMap<i64, String>,
    buyer_names: &HashMap<i64, String>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &POQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "采购订单" }
                div class="page-actions" {
                    @if can_create {
                        a class="btn btn-primary" href=(POCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建采购订单"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (po_table_fragment(result, supplier_names, buyer_names, suppliers, params, can_delete))
        }
    }
}

fn po_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<PurchaseOrder>,
    supplier_names: &HashMap<i64, String>,
    buyer_names: &HashMap<i64, String>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &POQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "部分收货", count: None },
        TabItem { value: "4".into(), label: "已收货", count: None },
        TabItem { value: "5".into(), label: "已关闭", count: None },
        TabItem { value: "6".into(), label: "已取消", count: None },
    ];

    let selected_supplier = params.supplier_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_range = params.date_range.as_deref().unwrap_or("");

    html! {
        div class="po-list-panel" {
            (status_tabs_with_param(POTablePath::PATH, "#po-data-card", "#po-filter-form", tabs, &active_value, "status"))

            // ── Filter Bar ──
            form class="filter-bar filter-form" id="po-filter-form"
                hx-get=(POTablePath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#po-data-card"
                hx-select="#po-data-card"
                hx-swap="outerHTML"
                hx-include="#po-filter-form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索采购单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="supplier_id" {
                    option value="" { "全部供应商" }
                    @for s in suppliers {
                        option value=(s.id) selected[selected_supplier == s.id.to_string()] { (s.name) }
                    }
                }
                select class="filter-select" name="date_range" {
                    option value="" selected[selected_range.is_empty()] { "订单日期" }
                    option value="7d" selected[selected_range == "7d"] { "最近7天" }
                    option value="30d" selected[selected_range == "30d"] { "最近30天" }
                    option value="3m" selected[selected_range == "3m"] { "最近3个月" }
                }
            }

            // ── Data Table ──
            div class="data-card" id="po-data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "订单编号" }
                                th { "供应商名称" }
                                th { "订单日期" }
                                th { "预计到货" }
                                th { "状态" }
                                th class="num-right" { "总金额" }
                                th { "业务员" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for o in &result.items {
                                (po_row(o, supplier_names, buyer_names, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无订单数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(POListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn po_row(
    o: &PurchaseOrder,
    supplier_names: &HashMap<i64, String>,
    buyer_names: &HashMap<i64, String>,
    can_delete: bool,
) -> Markup {
    let detail_path = PODetailPath { id: o.id };
    let delete_path = PODeletePath { id: o.id };
    let (status_text, status_class) = status_label(o.status);
    let supplier_name = supplier_names.get(&o.supplier_id).map(|s| s.as_str()).unwrap_or("—");
    let buyer_name = buyer_names.get(&o.operator_id).map(|s| s.as_str()).unwrap_or("—");
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = o.status == PurchaseOrderStatus::Draft;

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(&onclick) { (o.doc_number) }
            td onclick=(&onclick) { (supplier_name) }
            td class="mono" onclick=(&onclick) { (o.order_date.format("%Y-%m-%d")) }
            td class="mono" onclick=(&onclick) { (o.expected_delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into())) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="num-right mono" onclick=(&onclick) { (format!("{:.2}", o.total_amount)) }
            td onclick=(&onclick) { (buyer_name) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions" {
                        a class="row-action-btn" href=(detail_path.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="row-action-btn text-danger" title="删除"
                                hx-confirm="确认删除该采购订单吗？"
                                hx-post=(delete_path)
                                hx-target="closest tr"
                                hx-swap="outerHTML swap:0.5s" {
                                (icon::trash_icon("w-4 h-4"))
                            }
                        }
                    }
                }
            }
        }
    }
}
