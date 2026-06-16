//! Handler integration tests for Purchase Demand Pool module.

mod common;

use common::TestApp;

#[tokio::test]
async fn demand_pool_list_full_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/demand-pool").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("需求") || resp.body_contains("demand"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn demand_pool_list_htmx_fragment() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/demand-pool").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn demand_pool_create_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/demand-pool/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn demand_pool_create_page_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/demand-pool/create").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn demand_pool_supplier_detail() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/demand-pool/create/supplier-detail?supplier_id=129").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn demand_pool_demand_rows() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/demand-pool/demand-rows?product_id=13439").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}
