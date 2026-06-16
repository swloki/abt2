//! Handler integration tests for Purchase Order module.
//!
//! Tests HTTP handlers via tower::ServiceExt::oneshot — no browser needed.
//! Covers: list, detail, create page, create via form, workflow actions, edge cases.
//!
//! State-mutating tests (submit/cancel) create their own orders to avoid
//! cross-test interference.

mod common;

use common::TestApp;

// ── List ──

#[tokio::test]
async fn po_list_full_page() {
    let app = TestApp::new().await;

    let resp = app.get("/admin/purchase/orders").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("采购订单"));
    assert!(resp.body_contains("<html"), "non-HTMX → full document");
}

#[tokio::test]
async fn po_list_htmx_fragment() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"), "HTMX → fragment only");
    assert!(resp.body_contains("采购订单"));
}

#[tokio::test]
async fn po_list_status_filter() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders?status=1").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn po_list_supplier_filter() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders?supplier_id=129").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn po_list_pagination() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders?page=2").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn po_list_keyword_search() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders?keyword=PO-2026").await;
    assert!(resp.is_ok());
}

// ── Detail ──

#[tokio::test]
async fn po_detail_existing() {
    let app = TestApp::new().await;

    let resp = app.get("/admin/purchase/orders/32").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(resp.body_contains("订单详情"));
}

#[tokio::test]
async fn po_detail_htmx_fragment() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders/32").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

#[tokio::test]
async fn po_detail_not_found() {
    let app = TestApp::new().await;

    let resp = app.get("/admin/purchase/orders/999999").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

// ── Create page ──

#[tokio::test]
async fn po_create_page_full() {
    let app = TestApp::new().await;

    let resp = app.get("/admin/purchase/orders/create").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    // The create page should contain form-related content
    assert!(resp.body_contains("供应商") || resp.body_contains("supplier"));
}

#[tokio::test]
async fn po_create_page_htmx() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders/create").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"));
}

// ── Create page sub-endpoints (HTMX fragments) ──

#[tokio::test]
async fn po_supplier_detail_fragment() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders/create/supplier-detail?supplier_id=129").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

#[tokio::test]
async fn po_product_search() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders/products?name=2835").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn po_product_search_by_code() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders/products?code=3010134033").await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn po_tax_rates_json() {
    let app = TestApp::new().await;

    let resp = app.get("/admin/purchase/orders/tax-rates").await;
    assert!(resp.is_ok());
    assert!(resp.body_contains("[") || resp.body_contains("{"), "should be JSON");
}

#[tokio::test]
async fn po_item_row_fragment() {
    let app = TestApp::new().await;

    let resp = app.get_htmx("/admin/purchase/orders/create/item-row?product_id=565").await;
    assert!(resp.is_ok(), "status {}", resp.status);
}

// ── Create via form + workflow actions (self-contained) ──

/// Helper: create a draft PO and return its ID.
async fn create_draft_po(app: &TestApp) -> i64 {
    let items_json = urlencoding(
        r#"[{"product_id":"565","description":"测试物料","quantity":"10","unit_price":"1.50","item_delivery_date":null,"discount_pct":null,"tax_rate_id":null}]"#,
    );
    let body = format!("supplier_id=129&order_date=2026-06-16&items_json={items_json}&currency=CNY");

    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "create returned {} body: {}", resp.status, &resp.body[..300.min(resp.body.len())]);

    let redirect = resp.hx_redirect().expect("should have HX-Redirect after create");
    redirect
        .trim_start_matches("/admin/purchase/orders/")
        .parse::<i64>()
        .expect("redirect should contain numeric order id")
}

#[tokio::test]
async fn po_create_via_form() {
    let app = TestApp::new().await;

    let id = create_draft_po(&app).await;
    assert!(id > 0, "created PO should have positive id");

    // Verify the new order is accessible
    let resp = app.get_htmx(&format!("/admin/purchase/orders/{id}")).await;
    assert!(resp.is_ok(), "newly created PO detail returned {}", resp.status);
}

#[tokio::test]
async fn po_create_invalid_date() {
    let app = TestApp::new().await;

    let body = "supplier_id=129&order_date=invalid&items_json=%5B%5D";
    let resp = app.post_htmx("/admin/purchase/orders/create", body).await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn po_edit_draft_order() {
    let app = TestApp::new().await;

    let id = create_draft_po(&app).await;

    let resp = app.get(&format!("/admin/purchase/orders/{id}/edit")).await;
    assert!(resp.is_ok(), "edit draft returned {} body: {}", resp.status, &resp.body[..200.min(resp.body.len())]);
}

#[tokio::test]
async fn po_submit_draft_order() {
    let app = TestApp::new().await;

    let id = create_draft_po(&app).await;

    let resp = app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    assert!(
        resp.is_ok() || resp.hx_redirect().is_some(),
        "submit returned {} body: {}",
        resp.status,
        &resp.body[..200.min(resp.body.len())]
    );
}

#[tokio::test]
async fn po_cancel_order() {
    let app = TestApp::new().await;

    let id = create_draft_po(&app).await;

    let resp = app.post_htmx(&format!("/admin/purchase/orders/{id}/cancel"), "").await;
    assert!(
        resp.is_ok() || resp.hx_redirect().is_some(),
        "cancel returned {} body: {}",
        resp.status,
        &resp.body[..200.min(resp.body.len())]
    );
}

#[tokio::test]
async fn po_edit_non_draft_rejected() {
    let app = TestApp::new().await;

    let id = create_draft_po(&app).await;

    // Submit it first
    let resp = app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    assert!(resp.is_ok(), "submit should work");

    // Now editing should fail (not draft)
    let resp = app.get(&format!("/admin/purchase/orders/{id}/edit")).await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn po_action_nonexistent_order() {
    let app = TestApp::new().await;

    let resp = app.post_htmx("/admin/purchase/orders/999999/submit", "").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// URL-encode form values to avoid breaking the body.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}
