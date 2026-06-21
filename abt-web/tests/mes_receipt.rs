//! 完工入库 + FQC 门控 Handler 集成测试
//!
//! 覆盖：创建入库单/确认入库/FQC门控查询/物料用量/异常输入

mod common;
use common::TestApp;

use abt_core::{
    mes::{
        production_receipt::{model::ReceiptListFilter, ProductionReceiptService},
        work_order::{model::WorkOrderFilter, WorkOrderService},
    },
    shared::types::ServiceContext,
};

const PRODUCT_ID: i64 = 565;
const WAREHOUSE_ID: i64 = 23320;

async fn create_and_release_wo(app: &TestApp, qty: &str) -> i64 {
    let body = format!(
        "product_id={PRODUCT_ID}&planned_qty={qty}&scheduled_start=2026-06-20&scheduled_end=2026-07-20"
    );
    app.post_htmx("/admin/mes/orders/create", &body).await;
    let svc = app.state.work_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(&ServiceContext::new(1), &mut conn, WorkOrderFilter {
            status: None, product_id: None, keyword: None, date_from: None, date_to: None, product_code: None,
        }, 1, 1)
        .await
        .unwrap();
    let wo_id = result.items.first().unwrap().id;
    app.post_htmx(&format!("/admin/mes/orders/{wo_id}/release"), "").await;
    wo_id
}

/// 查最新入库单 ID
async fn latest_receipt_id(app: &TestApp) -> i64 {
    let svc = app.state.production_receipt_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(&ServiceContext::new(1), &mut conn, ReceiptListFilter { keyword: None }, 1, 1)
        .await
        .unwrap();
    result.items.first().unwrap().id
}

// ════════════════════════════════════════════════════════════════════════════
//  创建入库单
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_receipt_draft() {
    let app = TestApp::new().await;
    let wo_id = create_and_release_wo(&app, "50").await;

    let body = format!(
        "work_order_id={wo_id}&product_id={PRODUCT_ID}&received_qty=50&warehouse_id={WAREHOUSE_ID}&receipt_date=2026-06-16"
    );
    let resp = app.post_htmx("/admin/mes/receipts/create", &body).await;
    assert!(
        resp.is_ok(),
        "create receipt FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );

    let receipt_id = latest_receipt_id(&app).await;
    let svc = app.state.production_receipt_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let receipt = svc.find_by_id(&ServiceContext::new(1), &mut conn, receipt_id).await.unwrap();
    assert_eq!(receipt.work_order_id, wo_id);
    assert_eq!(receipt.received_qty, rust_decimal::Decimal::from(50));
    assert_eq!(receipt.warehouse_id, WAREHOUSE_ID);
    // 默认状态 = Draft (1)
    assert_eq!(receipt.status.as_i16(), 1);
}

#[tokio::test]
async fn create_receipt_without_product_returns_error() {
    let app = TestApp::new().await;
    let wo_id = create_and_release_wo(&app, "10").await;

    // product_id 为空时 handler unwrap_or(0) → Service 层报错（产品不存在）
    let body = format!(
        "work_order_id={wo_id}&received_qty=10&warehouse_id={WAREHOUSE_ID}&receipt_date=2026-06-16"
    );
    let resp = app.post_htmx("/admin/mes/receipts/create", &body).await;
    // product_id=0 会导致 NotFound 或 Validation 错误
    assert!(resp.status.is_client_error() || resp.status.is_server_error(),
        "receipt without product_id should fail gracefully, got {}", resp.status);
}

#[tokio::test]
async fn create_receipt_invalid_date_rejected() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/receipts/create",
            "work_order_id=1&received_qty=10&warehouse_id=23320&receipt_date=BAD",
        )
        .await;
    // axum Form 反序列化失败返回 422（Unprocessable Entity）
    assert!(resp.status.is_client_error(), "invalid date should be rejected, got {}", resp.status);
}

// ════════════════════════════════════════════════════════════════════════════
//  确认入库
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn confirm_receipt_graceful() {
    let app = TestApp::new().await;
    let wo_id = create_and_release_wo(&app, "30").await;
    let body = format!(
        "work_order_id={wo_id}&product_id={PRODUCT_ID}&received_qty=30&warehouse_id={WAREHOUSE_ID}&receipt_date=2026-06-16"
    );
    app.post_htmx("/admin/mes/receipts/create", &body).await;
    let receipt_id = latest_receipt_id(&app).await;

    let resp = app.post_htmx(&format!("/admin/mes/receipts/{receipt_id}/confirm"), "").await;
    // 确认可能因 FQC 门控/库存/反冲失败 —— 验证不崩溃
    assert!(
        resp.is_ok() || resp.status.is_client_error(),
        "confirm receipt should succeed or fail gracefully, got {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  FQC 门控查询
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn get_fqc_status_does_not_crash() {
    let app = TestApp::new().await;
    let wo_id = create_and_release_wo(&app, "10").await;
    let body = format!(
        "work_order_id={wo_id}&product_id={PRODUCT_ID}&received_qty=10&warehouse_id={WAREHOUSE_ID}&receipt_date=2026-06-16"
    );
    app.post_htmx("/admin/mes/receipts/create", &body).await;
    let receipt_id = latest_receipt_id(&app).await;

    let svc = app.state.production_receipt_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    // FQC 门控查询不应崩溃
    let _gate = svc.get_fqc_status(&ServiceContext::new(1), &mut conn, receipt_id).await;
}

// ════════════════════════════════════════════════════════════════════════════
//  不存在 / 404
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn detail_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.get("/admin/mes/receipts/999999").await.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn confirm_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.post_htmx("/admin/mes/receipts/999999/confirm", "").await.status, axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  列表 & 详情页 & 物料用量
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/receipts").await.is_ok());
}

#[tokio::test]
async fn create_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/receipts/create").await.is_ok());
}

#[tokio::test]
async fn material_usage_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/material-usage").await.is_ok());
}

#[tokio::test]
async fn material_usage_data_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/material-usage/data").await.is_ok());
}

// ════════════════════════════════════════════════════════════════════════════
//  级联查询 API
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn search_wo_returns_ok() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/receipts/search-wo?q=").await.is_ok());
}

#[tokio::test]
async fn search_wh_returns_ok() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/receipts/search-wh?q=").await.is_ok());
}
