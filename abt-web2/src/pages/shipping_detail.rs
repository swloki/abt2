use std::collections::HashMap;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use tower_sessions::Session;

use abt_core::sales::shipping_request::model::*;

use crate::auth::session::CURRENT_USER_KEY;
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::shipping::*;
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

fn status_label(s: ShippingStatus) -> (&'static str, &'static str) {
    match s {
        ShippingStatus::Draft => ("草稿", "status-draft"),
        ShippingStatus::Confirmed => ("已确认", "status-confirmed"),
        ShippingStatus::Picking => ("拣货中", "status-progress"),
        ShippingStatus::Shipped => ("已发货", "status-shipped"),
        ShippingStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

async fn fetch_shipping_request(conn: &mut sqlx::postgres::PgConnection, id: i64) -> Option<ShippingRequest> {
    sqlx::query_as::<sqlx::Postgres, ShippingRequest>(
        "SELECT id, doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id, created_at, updated_at, deleted_at FROM shipping_requests WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
    .ok()
    .flatten()
}

async fn fetch_shipping_items(conn: &mut sqlx::postgres::PgConnection, shipping_id: i64) -> Vec<ShippingRequestItem> {
    sqlx::query_as::<sqlx::Postgres, ShippingRequestItem>(
        "SELECT id, shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description FROM shipping_request_items WHERE shipping_request_id = $1 ORDER BY line_no",
    )
    .bind(shipping_id)
    .fetch_all(conn)
    .await
    .unwrap_or_default()
}

async fn resolve_product_names(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ShippingRequestItem],
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

// ── Handlers ──

pub async fn get_shipping_detail(
    path: ShippingDetailPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let shipping = fetch_shipping_request(&mut conn, path.id)
        .await
        .ok_or_else(|| AppError::Internal("发货单不存在".into()))?;

    let items = fetch_shipping_items(&mut conn, path.id).await;
    let customer_name = resolve_customer_name(&mut conn, shipping.customer_id).await;
    let order_number = resolve_order_number(&mut conn, shipping.order_id).await;
    let product_names = resolve_product_names(&mut conn, &items).await;

    let content = shipping_detail_page(&shipping, &items, &customer_name, &order_number, &product_names);
    let page_html = admin_page(
        &headers, "发货详情", &claims, "sales",
        &format!("{}/{}", ShippingListPath::PATH, path.id),
        "销售管理", Some("发货详情"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn confirm_shipping(
    path: ConfirmShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE shipping_requests SET status = 2, updated_at = NOW() WHERE id = $1 AND status = 1 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn pick_shipping(
    path: PickShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE shipping_requests SET status = 3, updated_at = NOW() WHERE id = $1 AND status = 2 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn ship_shipping(
    path: ShipShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE shipping_requests SET status = 4, updated_at = NOW() WHERE id = $1 AND status = 3 AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn cancel_shipping(
    path: CancelShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let _claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE shipping_requests SET status = 5, updated_at = NOW() WHERE id = $1 AND status IN (1, 2) AND deleted_at IS NULL")
        .bind(path.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: ShippingStatus) -> Markup {
    let steps: &[(&str, ShippingStatus)] = &[
        ("草稿", ShippingStatus::Draft),
        ("已确认", ShippingStatus::Confirmed),
        ("拣货中", ShippingStatus::Picking),
        ("已发货", ShippingStatus::Shipped),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == ShippingStatus::Cancelled;

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx && !is_cancelled { "workflow-connector active" } else { "workflow-connector" }) {}
                }
                @let step_class = if is_cancelled {
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
        }
    }
}

// ── Components ──

fn shipping_detail_page(
    s: &ShippingRequest,
    items: &[ShippingRequestItem],
    customer_name: &str,
    order_number: &str,
    product_names: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(s.status);

    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "发货详情" }
                div class="page-actions" {
                    a class="btn btn-default" href=(ShippingListPath::PATH) { "返回列表" }
                    @if s.status == ShippingStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(ConfirmShippingPath { id: s.id }.to_string())
                            hx-confirm="确认审核此发货单？" { "确认发货" }
                    }
                    @if s.status == ShippingStatus::Confirmed {
                        button class="btn btn-primary"
                            hx-post=(PickShippingPath { id: s.id }.to_string())
                            hx-confirm="确认开始拣货？" { "开始拣货" }
                    }
                    @if s.status == ShippingStatus::Picking {
                        button class="btn btn-success"
                            hx-post=(ShipShippingPath { id: s.id }.to_string())
                            hx-confirm="确认已发出？" { "确认发出" }
                    }
                    @if matches!(s.status, ShippingStatus::Draft | ShippingStatus::Confirmed) {
                        button class="btn btn-danger"
                            hx-post=(CancelShippingPath { id: s.id }.to_string())
                            hx-confirm="确认取消此发货单？" { "取消" }
                    }
                }
            }

            div class="data-card" style="margin-bottom:var(--space-6)" {
                (workflow_steps(s.status))
            }

            div class="data-card" style="margin-bottom:var(--space-6)" {
                div style="display:flex;align-items:center;gap:var(--space-4);margin-bottom:var(--space-4)" {
                    span class="mono" style="font-size:var(--text-xl);font-weight:600" { (s.doc_number) }
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
                        span class="info-label" { "申请日期" }
                        span class="info-value" { (s.request_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "预计发货" }
                        span class="info-value" {
                            (s.expected_ship_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "承运商" }
                        span class="info-value" { (s.carrier.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "物流单号" }
                        span class="info-value mono" { (s.tracking_number.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "收货地址" }
                        span class="info-value" { (s.shipping_address.as_str()) }
                    }
                    @if !s.remark.is_empty() {
                        div class="info-item" {
                            span class="info-label" { "备注" }
                            span class="info-value" { (s.remark.as_str()) }
                        }
                    }
                }
            }

            div class="data-card" {
                div class="form-section-title" { "发货明细" }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "序号" }
                                th { "产品" }
                                th { "描述" }
                                th class="num-right" { "申请数量" }
                                th class="num-right" { "已发货" }
                                th { "备注" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names))
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

fn item_row(item: &ShippingRequestItem, names: &HashMap<i64, String>) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td { (item.line_no) }
            td { (product_name) }
            td { (item.description.as_str()) }
            td class="num-right" { (item.requested_qty) }
            td class="num-right" { (item.shipped_qty) }
            td { }
        }
    }
}
