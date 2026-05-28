use std::collections::HashMap;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use tower_sessions::Session;

use abt_core::sales::shipping_request::model::*;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::ServiceContext;

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::icon;
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::shipping::*;
use crate::state::AppState;

// ── Helpers ──

fn make_ctx(operator_id: i64) -> ServiceContext {
    ServiceContext::new(operator_id)
}

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

async fn fetch_shipping_items(conn: &mut sqlx::postgres::PgConnection, shipping_id: i64) -> Vec<ShippingRequestItem> {
    sqlx::query_as::<sqlx::Postgres, ShippingRequestItem>(
        "SELECT id, shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description FROM shipping_request_items WHERE shipping_request_id = $1 ORDER BY line_no",
    )
    .bind(shipping_id)
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

async fn resolve_order_number(conn: &mut sqlx::postgres::PgConnection, order_id: i64) -> String {
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT doc_number FROM sales_orders WHERE id = $1")
        .bind(order_id)
        .fetch_one(conn)
        .await
        .unwrap_or_else(|_| "—".into())
}

async fn resolve_operator_name(conn: &mut sqlx::postgres::PgConnection, operator_id: i64) -> String {
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT COALESCE(display_name, username) FROM users WHERE id = $1")
        .bind(operator_id)
        .fetch_optional(conn)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "—".into())
}

struct ProductDetail {
    code: String,
    name: String,
    spec: Option<String>,
    unit: Option<String>,
}

async fn resolve_product_details(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ShippingRequestItem],
) -> HashMap<i64, ProductDetail> {
    let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT product_id, product_code, pdt_name, meta->>'specification', unit FROM products WHERE product_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(conn)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|(id, code, name, spec, unit)| (id, ProductDetail { code, name, spec, unit }))
        .collect()
}

async fn resolve_warehouse_names(
    conn: &mut sqlx::postgres::PgConnection,
    items: &[ShippingRequestItem],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|i| i.warehouse_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM warehouses WHERE id = ANY($1)")
        .bind(&ids)
        .fetch_all(conn)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect()
}

// ── Handlers ──

pub async fn get_shipping_detail(
    path: ShippingDetailPath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let svc = state.shipping_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(claims.sub);
    let shipping = svc.find_by_id(&ctx, &mut *conn, path.id).await.map_err(|e| AppError::Internal(e.to_string()))?;

    let items = fetch_shipping_items(&mut conn, path.id).await;
    let customer_name = resolve_customer_name(&mut conn, shipping.customer_id).await;
    let order_number = resolve_order_number(&mut conn, shipping.order_id).await;
    let operator_name = resolve_operator_name(&mut conn, shipping.operator_id).await;
    let product_details = resolve_product_details(&mut conn, &items).await;
    let warehouse_names = resolve_warehouse_names(&mut conn, &items).await;

    let content = shipping_detail_page(&shipping, &items, &customer_name, &order_number, &operator_name, &product_details, &warehouse_names);
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
    let claims = get_claims(&session).await;
    let svc = state.shipping_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(claims.sub);
    svc.confirm(&ctx, &mut *conn, path.id).await.map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn pick_shipping(
    path: PickShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let claims = get_claims(&session).await;
    let svc = state.shipping_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(claims.sub);
    svc.pick(&ctx, &mut *conn, path.id).await.map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn ship_shipping(
    path: ShipShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let claims = get_claims(&session).await;
    let svc = state.shipping_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(claims.sub);
    svc.ship(&ctx, &mut *conn, path.id).await.map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ShippingDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn cancel_shipping(
    path: CancelShippingPath,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let claims = get_claims(&session).await;
    let svc = state.shipping_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(claims.sub);
    svc.cancel(&ctx, &mut *conn, path.id).await.map_err(|e| AppError::Internal(e.to_string()))?;

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
    operator_name: &str,
    product_details: &HashMap<i64, ProductDetail>,
    warehouse_names: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(s.status);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(ShippingListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回发货申请列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (s.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                    div style="margin-top:var(--space-2);font-size:13px;color:var(--muted)" {
                        "来源订单："
                        a href=(format!("/admin/orders/{}", s.order_id)) style="color:var(--info);font-weight:500" { (order_number) }
                    }
                }
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

            // ── Workflow Steps ──
            (workflow_steps(s.status))

            // ── Shipping Info ──
            div class="info-card" {
                div class="info-card-title" { "发货信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "客户名称" }
                        span class="info-value" { (customer_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "收货地址" }
                        span class="info-value" { (s.shipping_address.as_str()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "预计发货日期" }
                        span class="info-value mono" {
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
                        span class="info-label" { "操作员" }
                        span class="info-value" { (operator_name) }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th { "单位" }
                                th class="num-right" { "申请数量" }
                                th class="num-right" { "已发货" }
                                th { "发货仓库" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_details, warehouse_names))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Remarks ──
            @if !s.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-6)" {
                    div class="info-card-title" { "备注" }
                    p class="text-muted" { (s.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &ShippingRequestItem,
    details: &HashMap<i64, ProductDetail>,
    warehouses: &HashMap<i64, String>,
) -> Markup {
    let detail = details.get(&item.product_id);
    let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
    let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
    let spec = detail.and_then(|d| d.spec.as_deref()).unwrap_or("—");
    let unit = detail.and_then(|d| d.unit.as_deref()).unwrap_or("—");
    let warehouse = warehouses.get(&item.warehouse_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td class="mono" { (item.line_no) }
            td class="mono" { (product_code) }
            td { (product_name) }
            td { (spec) }
            td { (unit) }
            td class="num-right" { (item.requested_qty) }
            td class="num-right" { (item.shipped_qty) }
            td { (warehouse) }
        }
    }
}
