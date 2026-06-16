//! Handler integration tests for Purchase Return module.

mod common;

use common::TestApp;

#[tokio::test]
async fn pr_list_full_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/returns").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("退货"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn pr_list_htmx_fragment() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/returns").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn pr_list_status_filter() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/returns?status=1").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn pr_list_supplier_filter() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/returns?supplier_id=129").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn pr_create_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/returns/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn pr_create_page_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/returns/create").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn pr_order_items() {
    let app = TestApp::new().await;
    // Get items for order 32 (confirmed status)
    let resp = app.get_htmx("/admin/purchase/returns/order-items?order_id=32").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn pr_detail_not_found() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/returns/999999").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}
