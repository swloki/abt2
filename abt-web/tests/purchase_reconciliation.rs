//! Handler integration tests for Purchase Reconciliation module.

mod common;

use common::TestApp;

#[tokio::test]
async fn precon_list_full_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/reconciliations").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("对账") || resp.body_contains("recon"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn precon_list_htmx_fragment() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/reconciliations").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn precon_create_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/reconciliations/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn precon_create_page_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/reconciliations/create").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn precon_detail_not_found() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/reconciliations/999999").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}
