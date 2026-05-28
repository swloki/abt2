use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use tower_sessions::Session;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::shipping_request::model::*;
use abt_core::shared::types::PageParams;

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::shipping::*;
use crate::state::AppState;

// ── Query Params ──

fn empty_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::de::Deserializer<'de>,
    T: std::str::FromStr,
{
    let s: Option<String> = Option::deserialize(de)?;
    match s.as_deref() {
        None | Some("") => Ok(None),
        Some(v) => v.parse::<T>().map(Some).map_err(|_| {
            serde::de::Error::custom(format!("cannot parse '{v}'"))
        }),
    }
}

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

async fn get_claims(session: &Session) -> abt_core::shared::identity::model::Claims {
    session
        .get(CURRENT_USER_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| abt_core::shared::identity::model::Claims {
            sub: 0,
            username: "未知用户".into(),
            display_name: "未知用户".into(),
            system_role: "user".into(),
            role_ids: vec![],
            role_codes: vec![],
            department_ids: vec![],
            iss: String::new(),
            exp: 0,
            iat: 0,
        })
}

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
        ShippingStatus::Draft => ("草稿", "status-draft"),
        ShippingStatus::Confirmed => ("已确认", "status-confirmed"),
        ShippingStatus::Picking => ("拣货中", "status-progress"),
        ShippingStatus::Shipped => ("已发货", "status-shipped"),
        ShippingStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

async fn query_shipping_requests(
    conn: &mut sqlx::postgres::PgConnection,
    params: &ShippingQueryParams,
) -> abt_core::shared::types::PaginatedResult<ShippingRequest> {
    let page_num = params.page.unwrap_or(1);
    let page_size = 20u32;
    let offset = (page_num - 1) * page_size;

    let status_val = params.status;
    let keyword_val = params.keyword.as_deref();
    let customer_val = params.customer_id;

    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM shipping_requests
           WHERE deleted_at IS NULL
             AND ($1::smallint IS NULL OR status = $1)
             AND ($2::text IS NULL OR doc_number ILIKE '%' || $2 || '%')
             AND ($3::bigint IS NULL OR customer_id = $3)"#,
    )
    .bind(status_val)
    .bind(keyword_val)
    .bind(customer_val)
    .fetch_one(&mut *conn)
    .await
    .unwrap_or(0);

    let items: Vec<ShippingRequest> = sqlx::query_as(
        r#"SELECT id, doc_number, order_id, customer_id, request_date, expected_ship_date,
                  status, shipping_address, carrier, tracking_number, remark,
                  operator_id, created_at, updated_at, deleted_at
           FROM shipping_requests
           WHERE deleted_at IS NULL
             AND ($1::smallint IS NULL OR status = $1)
             AND ($2::text IS NULL OR doc_number ILIKE '%' || $2 || '%')
             AND ($3::bigint IS NULL OR customer_id = $3)
           ORDER BY id DESC
           LIMIT $4 OFFSET $5"#,
    )
    .bind(status_val)
    .bind(keyword_val)
    .bind(customer_val)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&mut *conn)
    .await
    .unwrap_or_default();

    let total = count as u64;
    let total_pages = total.div_ceil(page_size as u64) as u32;
    abt_core::shared::types::PaginatedResult {
        items,
        total,
        page: page_num,
        total_pages,
        page_size,
    }
}

async fn resolve_customer_names_shipping(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ShippingRequest],
) -> std::collections::HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|i| i.customer_id).collect();
    if ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM customers WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(conn)
    .await
    .unwrap_or_default();
    rows.into_iter().collect()
}

async fn resolve_order_numbers(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ShippingRequest],
) -> std::collections::HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|i| i.order_id).collect();
    if ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, doc_number FROM sales_orders WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(conn)
    .await
    .unwrap_or_default();
    rows.into_iter().collect()
}

// ── Handlers ──

pub async fn get_shipping_list(
    _path: ShippingListPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(params): Query<ShippingQueryParams>,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let result = query_shipping_requests(&mut conn, &params).await;
    let customer_names = resolve_customer_names_shipping(&mut conn, &result.items).await;
    let order_numbers = resolve_order_numbers(&mut conn, &result.items).await;

    let ctx = abt_core::shared::types::ServiceContext::new(
        &mut conn as abt_core::shared::types::PgExecutor<'_>,
        claims.sub,
    );
    let customers = customer_svc
        .list(ctx, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let content = shipping_list_page(&claims, &result, &customer_names, &order_numbers, &customers.items, &params);
    let page_html = admin_page(
        &headers, "发货申请", &claims, "sales", ShippingListPath::PATH, "销售管理", Some("发货申请"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn get_shipping_table(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<ShippingQueryParams>,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let result = query_shipping_requests(&mut conn, &params).await;
    let customer_names = resolve_customer_names_shipping(&mut conn, &result.items).await;
    let order_numbers = resolve_order_numbers(&mut conn, &result.items).await;

    let ctx = abt_core::shared::types::ServiceContext::new(
        &mut conn as abt_core::shared::types::PgExecutor<'_>,
        claims.sub,
    );
    let customers = customer_svc
        .list(ctx, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(shipping_table_fragment(&result, &customer_names, &order_numbers, &customers.items, &params).into_string()))
}

// ── Components ──

fn shipping_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<ShippingRequest>,
    customer_names: &std::collections::HashMap<i64, String>,
    order_numbers: &std::collections::HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ShippingQueryParams,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "发货申请" }
            }
            (shipping_table_fragment(result, customer_names, order_numbers, customers, params))
        }
    }
}

fn shipping_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<ShippingRequest>,
    customer_names: &std::collections::HashMap<i64, String>,
    order_numbers: &std::collections::HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ShippingQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "拣货中", count: None },
        TabItem { value: "4".into(), label: "已发货", count: None },
        TabItem { value: "5".into(), label: "已取消", count: None },
    ];

    let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();

    html! {
        div class="shipping-list-panel" {
            (status_tabs(ShippingTablePath::PATH, "closest .shipping-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索发货单号…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(ShippingTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .shipping-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="customer_id"
                    hx-get=(ShippingTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .shipping-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" { "全部客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
                    }
                }
            }

            div class="data-card" {
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
                                (shipping_row(s, customer_names, order_numbers))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
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
    customer_names: &std::collections::HashMap<i64, String>,
    order_numbers: &std::collections::HashMap<i64, String>,
) -> Markup {
    let detail_path = ShippingDetailPath { id: s.id };
    let (status_text, status_class) = status_label(s.status);
    let customer_name = customer_names.get(&s.customer_id).map(|n| n.as_str()).unwrap_or("—");
    let order_num = order_numbers.get(&s.order_id).map(|n| n.as_str()).unwrap_or("—");
    let ship_date = s.expected_ship_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
    let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (s.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { (order_num) }
            td onclick=(format!("location.href='{}'", detail_path)) { (customer_name) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { (ship_date) }
            td onclick=(format!("location.href='{}'", detail_path)) { (s.carrier.as_str()) }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) { (s.tracking_number.as_str()) }
            td onclick=(format!("location.href='{}'", detail_path)) { (created) }
            td onclick="event.stopPropagation()" {
                a class="row-action-btn" href=(detail_path.to_string()) title="查看详情" {
                    (icon::eye_icon("w-4 h-4"))
                }
            }
        }
    }
}
