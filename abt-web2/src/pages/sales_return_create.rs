use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Json};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::shared::types::{PageParams, PgExecutor, ServiceContext};

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::icon;
use crate::errors::AppError;
use crate::layout::page::admin_page;
use crate::routes::sales_return::*;
use crate::state::AppState;

// ── Helpers ──

fn make_ctx<'a>(conn: &'a mut sqlx::postgres::PgConnection, operator_id: i64) -> ServiceContext<'a> {
    ServiceContext::new(conn as PgExecutor<'a>, operator_id)
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

// ── Query ──

#[derive(Debug, Deserialize)]
pub struct OrderSearchParams {
    pub keyword: Option<String>,
}

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct ReturnCreateForm {
    pub order_id: i64,
    pub shipping_request_id: i64,
    pub customer_id: i64,
    pub return_reason: String,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ReturnItemWeb {
    order_item_id: i64,
    product_id: i64,
    returned_qty: String,
    disposition: i16,
}

// ── Handlers ──

pub async fn get_return_create(
    _path: ReturnCreatePath,
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let claims = get_claims(&session).await;
    let customer_svc = state.customer_service();
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let ctx = make_ctx(&mut conn, claims.sub);
    let customers = customer_svc
        .list(ctx, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let content = return_create_page(&customers.items);
    let page_html = admin_page(
        &headers, "新建退货单", &claims, "sales", ReturnCreatePath::PATH, "销售管理", Some("新建退货单"), content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search orders → return HTML fragment
pub async fn get_orders(
    State(state): State<AppState>,
    _session: Session,
    Query(params): Query<OrderSearchParams>,
) -> Result<Html<String>, AppError> {
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let keyword = params.keyword.as_deref().unwrap_or("");
    let pattern = format!("%{keyword}%");

    let orders: Vec<(i64, String, i64, i16)> = sqlx::query_as(
        r#"SELECT id, doc_number, customer_id, status
           FROM sales_orders
           WHERE deleted_at IS NULL
             AND status IN (2, 3, 4, 5)
             AND doc_number ILIKE $1
           ORDER BY id DESC
           LIMIT 20"#,
    )
    .bind(&pattern)
    .fetch_all(&mut *conn)
    .await
    .unwrap_or_default();

    // Resolve customer names
    let customer_ids: Vec<i64> = orders.iter().map(|o| o.2).collect();
    let customer_names: std::collections::HashMap<i64, String> = if customer_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, name FROM customers WHERE id = ANY($1)",
        )
        .bind(&customer_ids)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();
        rows.into_iter().collect()
    };

    Ok(Html(order_list_fragment(&orders, &customer_names).into_string()))
}

/// HTMX: load order items for return → returns JSON
pub async fn get_order_items(
    State(state): State<AppState>,
    _session: Session,
    Query(params): Query<ItemsByOrderParams>,
) -> Result<Json<OrderItemsResponse>, AppError> {
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let items: Vec<(i64, i32, i64, String, rust_decimal::Decimal, String, rust_decimal::Decimal)> = sqlx::query_as(
        r#"SELECT id, line_no, product_id, description, quantity, unit, unit_price
           FROM sales_order_items
           WHERE order_id = $1
           ORDER BY line_no"#,
    )
    .bind(params.order_id)
    .fetch_all(&mut *conn)
    .await
    .unwrap_or_default();

    let product_ids: Vec<i64> = items.iter().map(|i| i.2).collect();
    let product_names: std::collections::HashMap<i64, String> = if product_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT product_id, pdt_name FROM products WHERE product_id = ANY($1)",
        )
        .bind(&product_ids)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();
        rows.into_iter().collect()
    };

    let shipping_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM shipping_requests WHERE order_id = $1 AND status = 4 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .bind(params.order_id)
    .fetch_optional(&mut *conn)
    .await
    .unwrap_or(None);

    let items_json: Vec<OrderItemJson> = items.into_iter().map(|(id, _line_no, product_id, description, quantity, unit, unit_price)| {
        let product_name = product_names.get(&product_id).map(|s| s.as_str()).unwrap_or("—");
        OrderItemJson {
            order_item_id: id,
            product_id,
            product_name: product_name.to_string(),
            description,
            order_qty: quantity.to_string(),
            unit,
            unit_price: unit_price.to_string(),
        }
    }).collect();

    Ok(Json(OrderItemsResponse {
        items: items_json,
        shipping_id,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ItemsByOrderParams {
    pub order_id: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderItemsResponse {
    pub items: Vec<OrderItemJson>,
    pub shipping_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OrderItemJson {
    order_item_id: i64,
    product_id: i64,
    product_name: String,
    description: String,
    order_qty: String,
    unit: String,
    unit_price: String,
}

/// POST: create return from form submission
pub async fn create_return(
    _path: ReturnCreatePath,
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<ReturnCreateForm>,
) -> Result<impl IntoResponse, AppError> {
    let claims = get_claims(&session).await;
    let mut conn = state.pool.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

    let web_items: Vec<ReturnItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| AppError::BadRequest(format!("无效退货明细数据: {e}")))?;

    let total_amount: rust_decimal::Decimal = web_items.iter().map(|_| rust_decimal::Decimal::ZERO).sum();

    // Generate doc number
    let doc_number = format!("RMA-{}", chrono::Local::now().format("%Y%m%d%H%M%S"));

    let return_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_reason, total_amount, remark, operator_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id"#,
    )
    .bind(&doc_number)
    .bind(form.order_id)
    .bind(form.shipping_request_id)
    .bind(form.customer_id)
    .bind(&form.return_reason)
    .bind(total_amount)
    .bind(form.remark.as_deref().unwrap_or(""))
    .bind(claims.sub)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    // Insert items
    for item in &web_items {
        let qty: rust_decimal::Decimal = item.returned_qty.parse().unwrap_or(rust_decimal::Decimal::ONE);
        let unit_price: rust_decimal::Decimal = sqlx::query_scalar(
            "SELECT unit_price FROM sales_order_items WHERE id = $1",
        )
        .bind(item.order_item_id)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(rust_decimal::Decimal::ZERO);

        let amount = qty * unit_price;
        sqlx::query(
            r#"INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(return_id)
        .bind(item.order_item_id)
        .bind(item.product_id)
        .bind(qty)
        .bind(unit_price)
        .bind(amount)
        .bind(item.disposition)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    // Update total amount
    let total: rust_decimal::Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM sales_return_items WHERE return_id = $1",
    )
    .bind(return_id)
    .fetch_one(&mut *conn)
    .await
    .unwrap_or(rust_decimal::Decimal::ZERO);

    sqlx::query("UPDATE sales_returns SET total_amount = $2 WHERE id = $1")
        .bind(return_id)
        .bind(total)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let redirect = ReturnDetailPath { id: return_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn return_create_page(_customers: &[abt_core::master_data::customer::model::Customer]) -> Markup {
    html! {
        div x-data="returnForm()" {
            div class="page-header" {
                a class="back-link" href=(ReturnListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回退货列表"
                }
                h1 class="page-title" { "新建退货单" }
            }

            form id="return-form"
                  hx-post=(ReturnCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="items_json" x-model="itemsJson";
                input type="hidden" name="order_id" x-model="selectedOrderId";
                input type="hidden" name="shipping_request_id" x-model="shippingRequestId";
                input type="hidden" name="customer_id" x-model="selectedCustomerId";

                // ── Related Order ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "关联单据" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "搜索订单" }
                            input class="form-input" type="text" placeholder="输入订单号搜索…"
                                hx-get=(ReturnOrdersPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#order-search-results"
                                hx-swap="innerHTML"
                                hx-include="this" {}
                        }
                        div class="form-field" {
                            label { "退货原因" span style="color:var(--danger)" { "*" } }
                            select x-model="returnReason" name="return_reason" {
                                option value="" { "请选择" }
                                option value="质量问题" { "质量问题" }
                                option value="数量不符" { "数量不符" }
                                option value="规格错误" { "规格错误" }
                                option value="客户取消" { "客户取消" }
                                option value="其他" { "其他" }
                            }
                        }
                    }
                    div id="order-search-results" {}
                }

                // ── Return Items (loaded dynamically) ──
                div id="return-items-section" x-show="selectedOrderId" {
                    div class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                        div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                            span class="form-section-title" style="margin:0;padding:0;border:none" { "退货明细" }
                        }
                        div style="overflow-x:auto" {
                            table class="data-table" style="min-width:700px" {
                                thead {
                                    tr {
                                        th { "产品" }
                                        th class="num-right" { "订单数量" }
                                        th class="num-right" { "单价" }
                                        th style="width:100px;text-align:right" { "退货数量" }
                                        th style="width:120px" { "处理方式" }
                                        th { }
                                    }
                                }
                                tbody {
                                    template x-for="(item, idx) in items" {
                                        tr {
                                            td x-text="item.product_name" {}
                                            td class="num-right" x-text="item.order_qty" {}
                                            td class="num-right mono" x-text="'¥ ' + parseFloat(item.unit_price).toFixed(2)" {}
                                            td { input class="form-input" type="number" x-model="item.returned_qty" min="1" style="width:80px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                                            td {
                                                select x-model="item.disposition" style="width:120px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {
                                                    option value="1" { "退回库存" }
                                                    option value="2" { "报废" }
                                                    option value="3" { "返工" }
                                                }
                                            }
                                            td { button type="button" class="btn-remove-row" x-on:click="removeItem(idx)" title="删除" {
                                                (icon::x_icon("w-3.5 h-3.5"))
                                            } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Remark ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "备注" }
                    textarea name="remark" placeholder="输入退货备注…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(ReturnListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-primary" {
                        "提交退货"
                    }
                }
            }

            script src="/return-create.js" {}
        }
    }
}

fn order_list_fragment(
    orders: &[(i64, String, i64, i16)],
    customer_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    html! {
        @if orders.is_empty() {
            div style="text-align:center;padding:var(--space-4);color:var(--muted);font-size:var(--text-sm)" {
                "未找到匹配的订单"
            }
        } @else {
            div class="product-select-list" {
                @for (id, doc_number, customer_id, _status) in orders {
                    @let customer_name = customer_names.get(customer_id).map(|s| s.as_str()).unwrap_or("—");
                    @let order_json = serde_json::json!({
                        "order_id": id,
                        "doc_number": doc_number,
                        "customer_id": customer_id,
                        "customer_name": customer_name,
                    }).to_string();
                    div class="product-select-item" {
                        div class="product-select-info" {
                            div class="product-select-name" { (doc_number) }
                            div class="product-select-meta" {
                                span { (customer_name) }
                            }
                        }
                        button type="button" class="btn btn-sm btn-primary"
                            data-order=(order_json)
                            x-on:click="selectOrder(JSON.parse($el.dataset.order))" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}
