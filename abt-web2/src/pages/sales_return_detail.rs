use std::collections::HashMap;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use tower_sessions::Session;

use abt_core::sales::sales_return::model::*;

use crate::auth::session::CURRENT_USER_KEY;
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::sales_return::*;
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

fn status_label(s: ReturnStatus) -> (&'static str, &'static str) {
    match s {
        ReturnStatus::Draft => ("草稿", "status-draft"),
        ReturnStatus::Confirmed => ("已确认", "status-confirmed"),
        ReturnStatus::Received => ("已收货", "status-progress"),
        ReturnStatus::Inspecting => ("质检中", "status-progress"),
        ReturnStatus::Completed => ("已完成", "status-completed"),
        ReturnStatus::Cancelled => ("已取消", "status-cancelled"),
        ReturnStatus::Rejected => ("已驳回", "status-rejected"),
    }
}

fn disposition_label(d: ReturnDisposition) -> &'static str {
    match d {
        ReturnDisposition::Restock => "退回库存",
        ReturnDisposition::Scrap => "报废",
        ReturnDisposition::Rework => "返工",
    }
}

async fn fetch_sales_return(conn: &mut sqlx::postgres::PgConnection, id: i64) -> Option<SalesReturn> {
    sqlx::query_as::<sqlx::Postgres, SalesReturn>(
        "SELECT id, doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id, created_at, updated_at, deleted_at FROM sales_returns WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
    .ok()
    .flatten()
}

async fn fetch_return_items(conn: &mut sqlx::postgres::PgConnection, return_id: i64) -> Vec<SalesReturnItem> {
    sqlx::query_as::<sqlx::Postgres, SalesReturnItem>(
        "SELECT id, return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition FROM sales_return_items WHERE return_id = $1 ORDER BY id",
    )
    .bind(return_id)
    .fetch_all(conn)
    .await
    .unwrap_or_default()
}

async fn resolve_product_names(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[SalesReturnItem],
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

async fn resolve_customer_name(conn: &mut sqlx::postgres::PgConnection, customer_id: i64) -> String {
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT name FROM customers WHERE id = $1")
        .bind(customer_id)
        .fetch_one(conn)
        .await
        .unwrap_or_else(|_| "未知客户".into())
}

async fn resolve_order_number(conn: &mut sqlx::postgres::PgConnection, order_id: i64) -> String {
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT doc_number FROM sales_orders WHERE id = $1")
        .bind(order_id)
        .fetch_one(conn)
        .await
        .unwrap_or_else(|_| "—".into())
}

async fn resolve_shipping_number(conn: &mut sqlx::postgres::PgConnection, shipping_id: i64) -> String {
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT doc_number FROM shipping_requests WHERE id = $1")
        .bind(shipping_id)
        .fetch_one(conn)
        .await
        .unwrap_or_else(|_| "—".into())
}

// ── Handlers ──

pub async fn get_return_detail(
    path: ReturnDetailPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ret = fetch_sales_return(&mut conn, path.id)
        .await
        .ok_or_else(|| AppError::Internal("退货单不存在".into()))?;

    let items = fetch_return_items(&mut conn, path.id).await;
    let customer_name = resolve_customer_name(&mut conn, ret.customer_id).await;
    let order_number = resolve_order_number(&mut conn, ret.order_id).await;
    let shipping_number = resolve_shipping_number(&mut conn, ret.shipping_request_id).await;
    let product_names = resolve_product_names(&mut conn, &items).await;

    let content = return_detail_page(&ret, &items, &customer_name, &order_number, &shipping_number, &product_names);
    let page_html = admin_page(
        &headers, "退货详情", &claims, "sales",
        &format!("{}/{}", ReturnListPath::PATH, path.id),
        "销售管理", Some("退货详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn confirm_return(
    path: ConfirmReturnPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE sales_returns SET status = 2, updated_at = NOW() WHERE id = $1 AND status = 1 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn receive_return(
    path: ReceiveReturnPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE sales_returns SET status = 3, updated_at = NOW() WHERE id = $1 AND status = 2 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn inspect_return(
    path: InspectReturnPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE sales_returns SET status = 4, updated_at = NOW() WHERE id = $1 AND status = 3 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn complete_return(
    path: CompleteReturnPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE sales_returns SET status = 5, updated_at = NOW() WHERE id = $1 AND status = 4 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn reject_return(
    path: RejectReturnPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE sales_returns SET status = 7, updated_at = NOW() WHERE id = $1 AND status IN (2, 3, 4) AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReturnDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: ReturnStatus) -> Markup {
    let steps: &[(&str, ReturnStatus)] = &[
        ("草稿", ReturnStatus::Draft),
        ("已确认", ReturnStatus::Confirmed),
        ("已收货", ReturnStatus::Received),
        ("质检中", ReturnStatus::Inspecting),
        ("已完成", ReturnStatus::Completed),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == ReturnStatus::Cancelled;
    let is_rejected = current == ReturnStatus::Rejected;
    let terminal = is_cancelled || is_rejected;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx && !terminal { "workflow-connector active" } else { "workflow-connector" }) {}
                }
                @let step_class = if terminal {
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
            @if is_cancelled {
                div class="workflow-step cancelled" {
                    div class="step-dot" {}
                    span class="step-label" { "已取消" }
                }
            }
            @if is_rejected {
                div class="workflow-step rejected" {
                    div class="step-dot" {}
                    span class="step-label" { "已驳回" }
                }
            }
        }
    }
}

// ── Components ──

fn return_detail_page(
    r: &SalesReturn,
    items: &[SalesReturnItem],
    customer_name: &str,
    order_number: &str,
    shipping_number: &str,
    product_names: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(r.status);

    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "退货详情" }
                div class="page-actions" {
                    a class="btn btn-default" href=(ReturnListPath::PATH) { "返回列表" }
                    @if r.status == ReturnStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(ConfirmReturnPath { id: r.id }.to_string())
                            hx-confirm="确认审核此退货单？" { "确认退货" }
                    }
                    @if r.status == ReturnStatus::Confirmed {
                        button class="btn btn-primary"
                            hx-post=(ReceiveReturnPath { id: r.id }.to_string())
                            hx-confirm="确认已收到退货？" { "确认收货" }
                    }
                    @if r.status == ReturnStatus::Received {
                        button class="btn btn-primary"
                            hx-post=(InspectReturnPath { id: r.id }.to_string())
                            hx-confirm="确认开始质检？" { "开始质检" }
                    }
                    @if r.status == ReturnStatus::Inspecting {
                        button class="btn btn-success"
                            hx-post=(CompleteReturnPath { id: r.id }.to_string())
                            hx-confirm="确认完成退货？" { "完成退货" }
                        button class="btn btn-danger"
                            hx-post=(RejectReturnPath { id: r.id }.to_string())
                            hx-confirm="确认驳回此退货？" { "驳回" }
                    }
                }
            }

            div class="data-card" style="margin-bottom:var(--space-6)" {
                (workflow_steps(r.status))
            }

            div class="data-card" style="margin-bottom:var(--space-6)" {
                div style="display:flex;align-items:center;gap:var(--space-4);margin-bottom:var(--space-4)" {
                    span class="mono" style="font-size:var(--text-xl);font-weight:600" { (r.doc_number) }
                    span class=(format!("status-pill {status_class}")) { (status_text) }
                }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "来源订单" }
                        span class="info-value mono" { (order_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "来源发货" }
                        span class="info-value mono" { (shipping_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "退货日期" }
                        span class="info-value" { (r.return_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "退货原因" }
                        span class="info-value" { (r.return_reason.as_str()) }
                    }
                    @if !r.remark.is_empty() {
                        div class="info-item" {
                            span class="info-label" { "备注" }
                            span class="info-value" { (r.remark.as_str()) }
                        }
                    }
                }
            }

            div class="data-card" {
                div class="form-section-title" { "退货明细" }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "产品" }
                                th class="num-right" { "单价" }
                                th class="num-right" { "退货数量" }
                                th class="num-right" { "退货金额" }
                                th { "处理方式" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="5" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
                div class="amount-summary" {
                    div class="amount-row" {
                        span { "退货总额" }
                        span class="mono" style="font-size:var(--text-lg);font-weight:600" {
                            "¥ " (format!("{:.2}", r.total_amount))
                        }
                    }
                }
            }
        }
    }
}

fn item_row(item: &SalesReturnItem, names: &HashMap<i64, String>) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td { (product_name) }
            td class="num-right mono" { (format!("{:.2}", item.unit_price)) }
            td class="num-right" { (item.returned_qty) }
            td class="num-right mono" { (format!("{:.2}", item.amount)) }
            td { (disposition_label(item.disposition)) }
        }
    }
}
