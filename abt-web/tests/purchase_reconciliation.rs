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

// ── Preview 端点（创建页「待对账明细」预览）──

#[tokio::test]
async fn precon_preview_no_supplier() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/reconciliations/preview").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("请先选择供应商"));
}

#[tokio::test]
async fn precon_preview_no_period() {
    let app = TestApp::new().await;
    let resp = app
        .get("/admin/purchase/reconciliations/preview?supplier_id=1")
        .await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("请先选择对账期间"));
}

/// period 格式非法 → service 宽松降级为空（200 + 空态），而非 400 toast。
#[tokio::test]
async fn precon_preview_bad_period() {
    let app = TestApp::new().await;
    let resp = app
        .get("/admin/purchase/reconciliations/preview?supplier_id=1&period=BAD")
        .await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("precon-preview-area"));
}

#[tokio::test]
async fn precon_preview_valid_empty() {
    let app = TestApp::new().await;
    let resp = app
        .get("/admin/purchase/reconciliations/preview?supplier_id=999999&period=2026-06")
        .await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("没有可对账"));
}

#[tokio::test]
async fn precon_preview_htmx() {
    let app = TestApp::new().await;
    let resp = app
        .get_htmx("/admin/purchase/reconciliations/preview?supplier_id=999999&period=2026-06")
        .await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}
