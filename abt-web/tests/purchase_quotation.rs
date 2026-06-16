//! Handler integration tests for Purchase Quotation module.

mod common;

use common::TestApp;

#[tokio::test]
async fn pq_list_full_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/quotations").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("报价单") || resp.body_contains("报价"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn pq_list_htmx_fragment() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn pq_list_status_filter() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations?status=2").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn pq_list_supplier_filter() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations?supplier_id=129").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn pq_list_pagination() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations?page=2").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn pq_detail_existing() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/quotations/6").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn pq_detail_htmx_fragment() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations/6").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn pq_detail_not_found() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/quotations/999999").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pq_create_page() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/quotations/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn pq_create_page_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations/create").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn pq_product_search() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations/products?name=2835").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn pq_supplier_contacts() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations/create/supplier-contacts?supplier_id=129").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn pq_item_row() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/quotations/create/item-row?product_id=565").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}
