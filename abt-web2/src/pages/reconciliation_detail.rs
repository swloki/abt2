use std::collections::HashMap;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use tower_sessions::Session;

use abt_core::sales::reconciliation::model::*;

use crate::auth::session::CURRENT_USER_KEY;
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::state::AppState;

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

fn status_label(s: ReconciliationStatus) -> (&'static str, &'static str) {
    match s {
        ReconciliationStatus::Draft => ("草稿", "status-draft"),
        ReconciliationStatus::Sent => ("已发送", "status-sent"),
        ReconciliationStatus::Confirmed => ("已确认", "status-confirmed"),
        ReconciliationStatus::Disputed => ("有异议", "status-disputed"),
        ReconciliationStatus::Settled => ("已结算", "status-completed"),
    }
}

async fn fetch_reconciliation(conn: &mut sqlx::postgres::PgConnection, id: i64) -> Option<Reconciliation> {
    sqlx::query_as::<sqlx::Postgres, Reconciliation>(
        "SELECT id, doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id, created_at, updated_at, deleted_at FROM reconciliations WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
    .ok()
    .flatten()
}

async fn fetch_rec_items(conn: &mut sqlx::postgres::PgConnection, rec_id: i64) -> Vec<ReconciliationItem> {
    sqlx::query_as::<sqlx::Postgres, ReconciliationItem>(
        "SELECT id, reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark FROM reconciliation_items WHERE reconciliation_id = $1 ORDER BY id",
    )
    .bind(rec_id)
    .fetch_all(conn)
    .await
    .unwrap_or_default()
}

async fn resolve_customer_name(conn: &mut sqlx::postgres::PgConnection, customer_id: i64) -> String {
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT name FROM customers WHERE id = $1")
        .bind(customer_id)
        .fetch_one(conn)
        .await
        .unwrap_or_else(|_| "未知客户".into())
}

async fn resolve_product_names_rec(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ReconciliationItem],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT product_id, pdt_name FROM products WHERE product_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(conn)
    .await
    .unwrap_or_default();
    rows.into_iter().collect()
}

async fn resolve_order_numbers(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ReconciliationItem],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|i| i.sales_order_id).collect();
    if ids.is_empty() {
        return HashMap::new();
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

pub async fn get_reconciliation_detail(
    path: ReconciliationDetailPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let rec = fetch_reconciliation(&mut conn, path.id)
        .await
        .ok_or_else(|| AppError::Internal("对账单不存在".into()))?;

    let items = fetch_rec_items(&mut conn, path.id).await;
    let customer_name = resolve_customer_name(&mut conn, rec.customer_id).await;
    let product_names = resolve_product_names_rec(&mut conn, &items).await;
    let order_numbers = resolve_order_numbers(&mut conn, &items).await;

    let content = reconciliation_detail_page(&rec, &items, &customer_name, &product_names, &order_numbers);
    let page_html = admin_page(
        &headers, "对账详情", &claims, "sales",
        &format!("{}/{}", ReconciliationListPath::PATH, path.id),
        "销售管理", Some("对账详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn send_reconciliation(
    path: SendReconciliationPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE reconciliations SET status = 2, updated_at = NOW() WHERE id = $1 AND status = 1 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn confirm_reconciliation(
    path: ConfirmReconciliationPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE reconciliations SET status = 3, updated_at = NOW() WHERE id = $1 AND status = 2 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn dispute_reconciliation(
    path: DisputeReconciliationPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE reconciliations SET status = 4, updated_at = NOW() WHERE id = $1 AND status IN (2, 3) AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn settle_reconciliation(
    path: SettleReconciliationPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE reconciliations SET status = 5, updated_at = NOW() WHERE id = $1 AND status = 3 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReconciliationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: ReconciliationStatus) -> Markup {
    let steps: &[(&str, ReconciliationStatus)] = &[
        ("草稿", ReconciliationStatus::Draft),
        ("已发送", ReconciliationStatus::Sent),
        ("已确认", ReconciliationStatus::Confirmed),
        ("已结算", ReconciliationStatus::Settled),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_disputed = current == ReconciliationStatus::Disputed;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx && !is_disputed { "workflow-connector active" } else { "workflow-connector" }) {}
                }
                @let step_class = if is_disputed {
                    "workflow-step"
                } else if i < current_idx {
                    "workflow-step completed"
                } else if i == current_idx {
                    "workflow-step active"
                } else {
                    "workflow-step"
                };
                div class=(step_class) {
                    div class="step-dot" {}
                    span class="step-label" { (label) }
                }
            }
            @if is_disputed {
                div class="workflow-step disputed" {
                    div class="step-dot" {}
                    span class="step-label" { "有异议" }
                }
            }
        }
    }
}

// ── Components ──

fn reconciliation_detail_page(
    rec: &Reconciliation,
    items: &[ReconciliationItem],
    customer_name: &str,
    product_names: &HashMap<i64, String>,
    order_numbers: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(rec.status);

    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "对账详情" }
                div class="page-actions" {
                    a class="btn btn-default" href=(ReconciliationListPath::PATH) { "返回列表" }
                    @if rec.status == ReconciliationStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(SendReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认发送此对账单？" { "发送对账" }
                    }
                    @if rec.status == ReconciliationStatus::Sent {
                        button class="btn btn-success"
                            hx-post=(ConfirmReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认此对账单？" { "确认" }
                        button class="btn btn-danger"
                            hx-post=(DisputeReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认提出异议？" { "异议" }
                    }
                    @if rec.status == ReconciliationStatus::Confirmed {
                        button class="btn btn-success"
                            hx-post=(SettleReconciliationPath { id: rec.id }.to_string())
                            hx-confirm="确认结算？" { "结算" }
                    }
                }
            }

            div class="data-card" style="margin-bottom:var(--space-6)" {
                (workflow_steps(rec.status))
            }

            // ── Summary Cards ──
            div style="display:grid;grid-template-columns:repeat(3,1fr);gap:var(--space-4);margin-bottom:var(--space-6)" {
                div class="data-card" style="text-align:center" {
                    div class="info-label" { "总金额" }
                    div class="mono" style="font-size:var(--text-xl);font-weight:600" {
                        "¥ " (format!("{:.2}", rec.total_amount))
                    }
                }
                div class="data-card" style="text-align:center" {
                    div class="info-label" { "确认金额" }
                    div class="mono" style="font-size:var(--text-xl);font-weight:600;color:var(--success)" {
                        "¥ " (format!("{:.2}", rec.confirmed_amount))
                    }
                }
                div class="data-card" style="text-align:center" {
                    div class="info-label" { "差额" }
                    div class="mono" style="font-size:var(--text-xl);font-weight:600;color:var(--danger)" {
                        "¥ " (format!("{:.2}", rec.difference))
                    }
                }
            }

            // ── Info ──
            div class="data-card" style="margin-bottom:var(--space-6)" {
                div style="display:flex;align-items:center;gap:var(--space-4);margin-bottom:var(--space-4)" {
                    span class="mono" style="font-size:var(--text-xl);font-weight:600" { (rec.doc_number) }
                    span class=(format!("status-pill {status_class}")) { (status_text) }
                }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "对账期间" }
                        span class="info-value" { (rec.period.as_str()) }
                    }
                    @if !rec.remark.is_empty() {
                        div class="info-item" {
                            span class="info-label" { "备注" }
                            span class="info-value" { (rec.remark.as_str()) }
                        }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                div class="form-section-title" { "对账明细" }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "产品" }
                                th { "来源订单" }
                                th class="num-right" { "数量" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "金额" }
                                th { "确认" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names, order_numbers))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="6" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn item_row(
    item: &ReconciliationItem,
    product_names: &HashMap<i64, String>,
    order_numbers: &HashMap<i64, String>,
) -> Markup {
    let product_name = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let order_num = order_numbers.get(&item.sales_order_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td { (product_name) }
            td class="mono" { (order_num) }
            td class="num-right" { (item.quantity) }
            td class="num-right mono" { (format!("{:.2}", item.unit_price)) }
            td class="num-right mono" { (format!("{:.2}", item.amount)) }
            td {
                @if item.confirmed {
                    span style="color:var(--success)" { "已确认" }
                } @else {
                    span style="color:var(--muted)" { "未确认" }
                }
            }
        }
    }
}
