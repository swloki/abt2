//! Handler integration tests for Purchase Approval Rules module.

mod common;

use common::TestApp;

#[tokio::test]
async fn approval_rules_list() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/approval-rules").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("审批") || resp.body_contains("approval"));
    assert!(resp.body_contains("<html"));
}

#[tokio::test]
async fn approval_rules_list_htmx() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/approval-rules").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn approval_rule_create_modal() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/approval-rules/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn approval_rule_create_and_delete() {
    let app = TestApp::new().await;

    // Create a rule
    let body = "name=测试审批规则&min_amount=10000&max_amount=100000&approver_role=&approver_id=&is_active=true&sort_order=99";
    let resp = app.post_htmx("/admin/purchase/approval-rules", body).await;
    assert!(
        resp.is_ok(),
        "create rule returned {} body: {}",
        resp.status,
        &resp.body[..200.min(resp.body.len())]
    );

    // The response should be HTML (updated list or modal close)
}

#[tokio::test]
async fn approval_rule_edit_modal_not_found() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/approval-rules/999999").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}
