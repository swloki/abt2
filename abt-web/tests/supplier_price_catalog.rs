//! Handler integration tests for Supplier Price Catalog module.

mod common;

use common::TestApp;

#[tokio::test]
async fn supplier_prices_list() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/supplier-prices").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("价格") || resp.body_contains("price"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn supplier_prices_list_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/supplier-prices").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn supplier_price_create_modal() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/supplier-prices/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn supplier_price_edit_modal_not_found() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/supplier-prices/999999").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}
