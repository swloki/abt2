use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use tower_sessions::Session;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::reconciliation::model::*;
use abt_core::shared::types::PageParams;

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
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
pub struct ReconciliationQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub customer_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub period: Option<String>,
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

fn build_query_string(params: &ReconciliationQueryParams) -> String {
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
    if let Some(ref p) = params.period {
        q.push(format!("period={p}"));
    }
    q.join("&")
}

fn status_label(s: ReconciliationStatus) -> (&'static str, &'static str) {
    match s {
        ReconciliationStatus::Draft => ("草稿", "status-draft"),
        ReconciliationStatus::Sent => ("已发送", "status-sent"),
        ReconciliationStatus::Confirmed => ("已确认", "status-confirmed"),
        ReconciliationStatus::Disputed => ("有异议", "status-disputed"),
        ReconciliationStatus::Settled => ("已结算", "status-completed"),
    }
}

async fn query_reconciliations(
    conn: &mut sqlx::postgres::PgConnection,
    params: &ReconciliationQueryParams,
) -> abt_core::shared::types::PaginatedResult<Reconciliation> {
    let page_num = params.page.unwrap_or(1);
    let page_size = 20u32;
    let offset = (page_num - 1) * page_size;

    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM reconciliations
           WHERE deleted_at IS NULL
             AND ($1::smallint IS NULL OR status = $1)
             AND ($2::text IS NULL OR doc_number ILIKE '%' || $2 || '%')
             AND ($3::bigint IS NULL OR customer_id = $3)
             AND ($4::text IS NULL OR period = $4)"#,
    )
    .bind(params.status)
    .bind(params.keyword.as_deref())
    .bind(params.customer_id)
    .bind(params.period.as_deref())
    .fetch_one(&mut *conn)
    .await
    .unwrap_or(0);

    let items: Vec<Reconciliation> = sqlx::query_as(
        r#"SELECT id, doc_number, customer_id, period, status,
                  total_amount, confirmed_amount, difference, remark,
                  operator_id, created_at, updated_at, deleted_at
           FROM reconciliations
           WHERE deleted_at IS NULL
             AND ($1::smallint IS NULL OR status = $1)
             AND ($2::text IS NULL OR doc_number ILIKE '%' || $2 || '%')
             AND ($3::bigint IS NULL OR customer_id = $3)
             AND ($4::text IS NULL OR period = $4)
           ORDER BY id DESC
           LIMIT $5 OFFSET $6"#,
    )
    .bind(params.status)
    .bind(params.keyword.as_deref())
    .bind(params.customer_id)
    .bind(params.period.as_deref())
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

async fn resolve_customer_names_rec(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[Reconciliation],
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

// ── Handlers ──

pub async fn get_reconciliation_list(
    _path: ReconciliationListPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(params): Query<ReconciliationQueryParams>,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let result = query_reconciliations(&mut conn, &params).await;
    let customer_names = resolve_customer_names_rec(&mut conn, &result.items).await;

    let ctx = abt_core::shared::types::ServiceContext::new(claims.sub);
    let customers = customer_svc
        .list(&ctx, &mut *conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let content = reconciliation_list_page(&claims, &result, &customer_names, &customers.items, &params);
    let page_html = admin_page(
        &headers, "月对账单", &claims, "sales", ReconciliationListPath::PATH, "销售管理", Some("月对账单"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn get_reconciliation_table(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<ReconciliationQueryParams>,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let result = query_reconciliations(&mut conn, &params).await;
    let customer_names = resolve_customer_names_rec(&mut conn, &result.items).await;

    let ctx = abt_core::shared::types::ServiceContext::new(claims.sub);
    let customers = customer_svc
        .list(&ctx, &mut *conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Html(reconciliation_table_fragment(&result, &customer_names, &customers.items, &params).into_string()))
}

// ── Components ──

fn reconciliation_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<Reconciliation>,
    customer_names: &std::collections::HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ReconciliationQueryParams,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "月对账单" }
            }
            (reconciliation_table_fragment(result, customer_names, customers, params))
        }
    }
}

fn reconciliation_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Reconciliation>,
    customer_names: &std::collections::HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ReconciliationQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已发送", count: None },
        TabItem { value: "3".into(), label: "已确认", count: None },
        TabItem { value: "4".into(), label: "有异议", count: None },
        TabItem { value: "5".into(), label: "已结算", count: None },
    ];

    let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_period = params.period.as_deref().unwrap_or("");

    html! {
        div class="reconciliation-list-panel" {
            (status_tabs(ReconciliationTablePath::PATH, "closest .reconciliation-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索对账单号…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(ReconciliationTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .reconciliation-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="customer_id"
                    hx-get=(ReconciliationTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .reconciliation-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" { "全部客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
                    }
                }
                select class="filter-select" name="period"
                    hx-get=(ReconciliationTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .reconciliation-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" selected[selected_period.is_empty()] { "对账期间" }
                    @for p in generate_periods() {
                        option value=(p.value) selected[selected_period == p.value] { (p.label) }
                    }
                }
            }

            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "对账单号" }
                                th { "客户名称" }
                                th { "对账期间" }
                                th class="num-right" { "总金额" }
                                th class="num-right" { "确认金额" }
                                th class="num-right" { "差额" }
                                th { "状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (reconciliation_row(r, customer_names))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无对账数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(ReconciliationListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

struct PeriodOption {
    value: &'static str,
    label: &'static str,
}

fn generate_periods() -> Vec<PeriodOption> {
    let now = chrono::Local::now();
    let mut periods = vec![];
    for i in 0..6 {
        let d = now - chrono::Months::new(i);
        let value = d.format("%Y-%m").to_string();
        periods.push(PeriodOption {
            value: Box::leak(value.into_boxed_str()),
            label: Box::leak(d.format("%Y年%m月").to_string().into_boxed_str()),
        });
    }
    periods
}

fn reconciliation_row(
    r: &Reconciliation,
    customer_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    let detail_path = ReconciliationDetailPath { id: r.id };
    let (status_text, status_class) = status_label(r.status);
    let customer_name = customer_names.get(&r.customer_id).map(|n| n.as_str()).unwrap_or("—");

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (r.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { (customer_name) }
            td onclick=(format!("location.href='{}'", detail_path)) { (r.period.as_str()) }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                span class="mono" { "¥ " (format!("{:.2}", r.total_amount)) }
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                span class="mono" { "¥ " (format!("{:.2}", r.confirmed_amount)) }
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                span class="mono" { "¥ " (format!("{:.2}", r.difference)) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick="event.stopPropagation()" {
                a class="row-action-btn" href=(detail_path.to_string()) title="查看详情" {
                    (icon::eye_icon("w-4 h-4"))
                }
            }
        }
    }
}
