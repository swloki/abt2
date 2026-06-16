//! Handler integration tests for Purchase Settings module.

mod common;

use common::TestApp;

#[tokio::test]
async fn settings_page_full() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/settings").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("设置") || resp.body_contains("settings"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn settings_page_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/settings").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn settings_update() {
    let app = TestApp::new().await;
    // Minimal update — just change over_delivery_allowance_pct
    let body = "over_delivery_allowance_pct=5";
    let resp = app.post_htmx("/admin/purchase/settings", body).await;
    assert!(
        resp.is_ok() || resp.hx_redirect().is_some(),
        "settings update returned {} body: {}",
        resp.status,
        &resp.body[..200.min(resp.body.len())]
    );
}
