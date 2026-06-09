use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;
use abt_core::shared::types::ServiceContext;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::utils::{empty_as_none, resolve_customer_names, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrderQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub customer_id: Option<i64>,
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

fn build_filter(params: &OrderQueryParams) -> SalesOrderQuery {
    let (date_from, date_to) = params
        .date_range
        .as_deref()
        .map(parse_date_range)
        .unwrap_or((None, None));
    SalesOrderQuery {
        keyword: params.keyword.clone(),
        status: params.status.and_then(SalesOrderStatus::from_i16),
        customer_id: params.customer_id,
        date_from,
        date_to,
    }
}

fn build_query_string(params: &OrderQueryParams) -> String {
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
    if let Some(ref dr) = params.date_range {
        q.push(format!("date_range={dr}"));
    }
    q.join("&")
}

async fn resolve_sales_rep_names<S: UserService>(
    svc: &S,
    ctx: &ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    orders: &[SalesOrder],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = orders.iter().map(|o| o.sales_rep_id).collect();
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

fn status_label(s: SalesOrderStatus) -> (&'static str, &'static str) {
    match s {
        SalesOrderStatus::Draft => ("草稿", "status-draft"),
        SalesOrderStatus::Confirmed => ("已确认", "status-confirmed"),
        SalesOrderStatus::InProduction => ("生产中", "status-progress"),
        SalesOrderStatus::PartiallyShipped => ("部分发货", "status-partial"),
        SalesOrderStatus::Shipped => ("已发货", "status-shipped"),
        SalesOrderStatus::Completed => ("已完成", "status-completed"),
        SalesOrderStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_list(
    _path: OrderListPath,
    ctx: RequestContext,
    Query(params): Query<OrderQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();
    let customer_svc = state.customer_service();
    let user_svc = state.user_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let customer_names = resolve_customer_names(&customer_svc, &service_ctx, &mut conn, result.items.iter().map(|o| o.customer_id)).await;
    let sales_rep_names = resolve_sales_rep_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let content = order_list_page(&result, &customer_names, &sales_rep_names, &customers.items, &params);
    let page_html = admin_page(
        is_htmx, "销售订单", &claims, "sales", OrderListPath::PATH, "销售管理", Some("销售订单"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_table(
    ctx: RequestContext,
    Query(params): Query<OrderQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();
    let customer_svc = state.customer_service();
    let user_svc = state.user_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let customer_names = resolve_customer_names(&customer_svc, &service_ctx, &mut conn, result.items.iter().map(|o| o.customer_id)).await;
    let sales_rep_names = resolve_sales_rep_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    Ok(Html(order_table_fragment(&result, &customer_names, &sales_rep_names, &customers.items, &params).into_string()))
}

// ── Edit / Delete Handlers ──

pub async fn delete_order(
    path: DeleteOrderPath,
    ctx: RequestContext,
) -> Result<impl axum::response::IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", OrderListPath::PATH)], Html(String::new())))
}

// ── Components ──

fn order_list_page(
    result: &abt_core::shared::types::PaginatedResult<SalesOrder>,
    customer_names: &HashMap<i64, String>,
    sales_rep_names: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &OrderQueryParams,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "销售订单" }
                div class="page-actions" {
                    button class="btn btn-default" {
                        (icon::download_icon("w-4 h-4"))
                        "导出"
                    }
                    a class="btn btn-primary" href=(OrderCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建订单"
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (order_table_fragment(result, customer_names, sales_rep_names, customers, params))
        }
    }
}

fn order_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<SalesOrder>,
    customer_names: &HashMap<i64, String>,
    sales_rep_names: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &OrderQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "生产中", count: None },
        TabItem { value: "4".into(), label: "部分发货", count: None },
        TabItem { value: "5".into(), label: "已发货", count: None },
        TabItem { value: "6".into(), label: "已完成", count: None },
        TabItem { value: "7".into(), label: "已取消", count: None },
    ];

    let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_range = params.date_range.as_deref().unwrap_or("");

    html! {
        div class="order-list-panel" {
            (status_tabs(OrderTablePath::PATH, "closest .order-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索订单号、客户名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(OrderTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .order-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="customer_id"
                    hx-get=(OrderTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .order-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" { "全部客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
                    }
                }
                select class="filter-select" name="date_range"
                    hx-get=(OrderTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .order-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" selected[selected_range.is_empty()] { "交货日期" }
                    option value="7d" selected[selected_range == "7d"] { "最近7天" }
                    option value="30d" selected[selected_range == "30d"] { "最近30天" }
                    option value="3m" selected[selected_range == "3m"] { "最近3个月" }
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "订单号" }
                                th { "来源报价" }
                                th { "客户名称" }
                                th { "状态" }
                                th class="num-right" { "总金额" }
                                th { "交货日期" }
                                th { "业务员" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for o in &result.items {
                                (order_row(o, customer_names, sales_rep_names))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="9" class="td-empty" {
                                        "暂无订单数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(OrderListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn order_row(
    o: &SalesOrder,
    customer_names: &HashMap<i64, String>,
    sales_rep_names: &HashMap<i64, String>,
) -> Markup {
    let detail_path = OrderDetailPath { id: o.id };
    let edit_form_path = OrderEditFormPath { id: o.id };
    let delete_path = DeleteOrderPath { id: o.id };
    let (status_text, status_class) = status_label(o.status);
    let customer_name = customer_names.get(&o.customer_id).map(|s| s.as_str()).unwrap_or("—");
    let sales_rep = sales_rep_names.get(&o.sales_rep_id).map(|s| s.as_str()).unwrap_or("—");
    let created = o.created_at.format("%Y-%m-%d").to_string();
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = o.status == SalesOrderStatus::Draft;

    html! {
        tr {
            td class="link-cell mono" onclick=(&onclick) { (o.doc_number) }
            td onclick=(&onclick) { "—" }
            td onclick=(&onclick) { (customer_name) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="num-right" onclick=(&onclick) {
                span class="mono" { "¥ " (format!("{:.2}", o.total_amount)) }
            }
            td class="mono" onclick=(&onclick) { (o.order_date.format("%Y-%m-%d")) }
            td onclick=(&onclick) { (sales_rep) }
            td onclick=(&onclick) { (created) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions" {
                        a class="row-action-btn" href=(edit_form_path.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        button type="button" class="row-action-btn text-danger" title="删除"
                            hx-confirm="确认删除该订单吗？"
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

