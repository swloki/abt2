use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use tower_sessions::Session;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::reconciliation::model::*;
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::shared::types::{PageParams, ServiceContext};

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::confirm_dialog::confirm_dialog;
use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::state::AppState;
use crate::utils::{empty_as_none, resolve_customer_names};

// ── Query Params ──

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

/// Compute status counts by calling ReconciliationService::list for each status with page_size=1.
async fn count_by_status<S: ReconciliationService>(
    svc: &S,
    ctx: &ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    customer_id: Option<i64>,
) -> HashMap<i16, u64> {
    let statuses = [
        (ReconciliationStatus::Draft, 1i16),
        (ReconciliationStatus::Sent, 2),
        (ReconciliationStatus::Confirmed, 3),
        (ReconciliationStatus::Disputed, 4),
        (ReconciliationStatus::Settled, 5),
    ];

    let mut counts = HashMap::new();
    for (status, code) in statuses {
        let filter = ReconciliationQuery {
            customer_id,
            period: None,
            status: Some(status),
            keyword: None,
        };
        let page = PageParams::new(1, 1);
        if let Ok(result) = svc.list(ctx, db, filter, page).await {
            counts.insert(code, result.total);
        }
    }

    // Total = sum of all per-status counts
    let total: u64 = counts.values().sum();
    counts.insert(0, total);

    counts
}

// ── Handlers ──

pub async fn get_reconciliation_list(
    _path: ReconciliationListPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(params): Query<ReconciliationQueryParams>,
) -> Result<Html<String>> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(DomainError::from)?;
    let ctx = ServiceContext::new(claims.sub);

    let reconciliation_svc = state.reconciliation_service();
    let customer_svc = state.customer_service();

    let filter = ReconciliationQuery {
        customer_id: params.customer_id,
        period: params.period.clone(),
        status: params.status.and_then(ReconciliationStatus::from_i16),
        keyword: params.keyword.clone(),
    };
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = reconciliation_svc.list(&ctx, &mut conn, filter, page).await?;

    let status_counts = count_by_status(&reconciliation_svc, &ctx, &mut conn, params.customer_id).await;
    let customer_names = resolve_customer_names(&customer_svc, &ctx, &mut conn, result.items.iter().map(|i| i.customer_id)).await;

    let customers = customer_svc
        .list(&ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let content = reconciliation_list_page(&claims, &result, &customer_names, &customers.items, &params, &status_counts);
    let page_html = admin_page(
        &headers, "月对账单", &claims, "sales", ReconciliationListPath::PATH, "销售管理", Some("月对账单"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn get_reconciliation_table(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<ReconciliationQueryParams>,
) -> Result<Html<String>> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(DomainError::from)?;
    let ctx = ServiceContext::new(claims.sub);

    let reconciliation_svc = state.reconciliation_service();
    let customer_svc = state.customer_service();

    let filter = ReconciliationQuery {
        customer_id: params.customer_id,
        period: params.period.clone(),
        status: params.status.and_then(ReconciliationStatus::from_i16),
        keyword: params.keyword.clone(),
    };
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = reconciliation_svc.list(&ctx, &mut conn, filter, page).await?;

    let status_counts = count_by_status(&reconciliation_svc, &ctx, &mut conn, params.customer_id).await;
    let customer_names = resolve_customer_names(&customer_svc, &ctx, &mut conn, result.items.iter().map(|i| i.customer_id)).await;

    let customers = customer_svc
        .list(&ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    Ok(Html(reconciliation_table_fragment(&result, &customer_names, &customers.items, &params, &status_counts).into_string()))
}

pub async fn delete_reconciliation(
    path: ReconciliationDeletePath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(DomainError::from)?;
    let ctx = ServiceContext::new(claims.sub);

    let reconciliation_svc = state.reconciliation_service();
    reconciliation_svc.delete(&ctx, &mut conn, path.id).await?;

    let redirect = ReconciliationListPath::PATH.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn get_reconciliation_create_placeholder(
    _path: ReconciliationCreatePath,
    State(_state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let _claims = get_claims(&session).await;
    Err(DomainError::business_rule("新建对账单功能开发中").into())
}

// ── Components ──

fn reconciliation_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<Reconciliation>,
    customer_names: &std::collections::HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ReconciliationQueryParams,
    status_counts: &HashMap<i16, u64>,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "月对账单" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(ReconciliationCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建对账单"
                    }
                }
            }
            (reconciliation_table_fragment(result, customer_names, customers, params, status_counts))
        }
    }
}

fn reconciliation_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Reconciliation>,
    customer_names: &std::collections::HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &ReconciliationQueryParams,
    status_counts: &HashMap<i16, u64>,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();

    let total_count: u64 = status_counts.values().sum();
    let draft_count = status_counts.get(&1).copied();
    let sent_count = status_counts.get(&2).copied();
    let confirmed_count = status_counts.get(&3).copied();
    let disputed_count = status_counts.get(&4).copied();
    let settled_count = status_counts.get(&5).copied();

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: draft_count },
        TabItem { value: "2".into(), label: "已发送", count: sent_count },
        TabItem { value: "3".into(), label: "已确认", count: confirmed_count },
        TabItem { value: "4".into(), label: "有异议", count: disputed_count },
        TabItem { value: "5".into(), label: "已结算", count: settled_count },
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
                        placeholder="搜索对账单号、客户名称…"
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
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = r.status == ReconciliationStatus::Draft;
    let form_id = format!("delete-rec-form-{}", r.id);
    let delete_path = ReconciliationDeletePath { id: r.id };

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(&onclick) { (r.doc_number) }
            td onclick=(&onclick) { (customer_name) }
            td onclick=(&onclick) { (r.period.as_str()) }
            td class="num-right" onclick=(&onclick) {
                span class="mono" { "¥ " (format!("{:.2}", r.total_amount)) }
            }
            td class="num-right" onclick=(&onclick) {
                span class="mono" { "¥ " (format!("{:.2}", r.confirmed_amount)) }
            }
            td class="num-right" onclick=(&onclick) {
                span class="mono" style="font-weight:600" { "¥ " (format!("{:.2}", r.difference)) }
            }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick="event.stopPropagation()" x-data=(format!("{{ deleteOpen: false }}")) {
                div class="row-actions" {
                    @if is_draft {
                        a class="row-action-btn" href=(detail_path.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        button type="button" class="row-action-btn text-danger" title="删除"
                            x-on:click="deleteOpen = true" {
                            (icon::trash_icon("w-4 h-4"))
                        }
                    } @else {
                        a class="row-action-btn" href=(detail_path.to_string()) title="查看详情" {
                            (icon::eye_icon("w-4 h-4"))
                        }
                    }
                }
                @if is_draft {
                    (confirm_dialog(
                        "deleteOpen",
                        "确认删除",
                        &format!("确定要删除对账单 <strong>{}</strong> 吗？", r.doc_number),
                        "确认删除",
                        &form_id,
                        html! {
                            form id=(form_id) style="display:none"
                                hx-post=(delete_path.to_string())
                                hx-target="closest tr"
                                hx-swap="outerHTML swap:0.5s" {}
                        },
                    ))
                }
            }
        }
    }
}
