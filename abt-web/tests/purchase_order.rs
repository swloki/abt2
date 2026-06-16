//! Deep handler integration tests — Purchase Order.
//!
//! Every mutating test: HTTP action → service-layer DB verification.
//! Asserts field-level correctness (status, supplier_id, amount_total),
//! not just "body contains text".

mod common;

use abt_core::{
    purchase::{
        enums::PurchaseOrderStatus,
        order::{PurchaseOrderService, model::PurchaseOrder},
    },
    shared::types::ServiceContext,
};
use common::TestApp;
use rust_decimal::{Decimal, prelude::FromPrimitive};

// ── Helpers ──

fn items_json(items: &[(&str, &str, &str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, desc, qty, price)| {
            format!(
                r#"{{"product_id":"{pid}","description":"{desc}","quantity":"{qty}","unit_price":"{price}","item_delivery_date":null,"discount_pct":null,"tax_rate_id":null}}"#
            )
        })
        .collect();
    urlencoding(&format!("[{}]", parts.join(",")))
}

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

async fn create_draft(app: &TestApp, supplier_id: i64, items_json: &str) -> i64 {
    let body = format!(
        "supplier_id={supplier_id}&order_date=2026-06-16&items_json={items_json}&currency=CNY"
    );
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(
        resp.is_ok(),
        "create PO failed: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    resp.hx_redirect()
        .expect("should have HX-Redirect")
        .trim_start_matches("/admin/purchase/orders/")
        .parse()
        .unwrap()
}

/// Fetch a PurchaseOrder via the service layer.
async fn get_order(app: &TestApp, id: i64) -> PurchaseOrder {
    let svc = app.state.purchase_order_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.get(&ctx, &mut conn, id).await.unwrap()
}

// ════════════════════════════════════════════════════════════════════════════
//  Create → verify DB state (field-level)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_single_item_correct_status_and_amount() {
    let app = TestApp::new().await;
    let items_json = items_json(&[("565", "深测-单品", "100", "12.50")]);
    let id = create_draft(&app, 129, &items_json).await;

    let order = get_order(&app, id).await;
    assert_eq!(order.status, PurchaseOrderStatus::Draft,
        "new PO must be Draft, got {:?}", order.status);
    assert_eq!(order.supplier_id, 129,
        "supplier must be 129, got {}", order.supplier_id);
    assert_eq!(order.amount_total, Decimal::from_i32(1250).unwrap(),
        "100 × 12.50 = 1250.00, got {}", order.amount_total);
    assert!(order.doc_number.starts_with("PO-"),
        "doc_number should start with PO-, got {}", order.doc_number);
}

#[tokio::test]
async fn create_three_items_correct_total() {
    let app = TestApp::new().await;
    let items_json = items_json(&[
        ("565", "灯珠2835", "1000", "0.15"),
        ("566", "灯珠2835W", "1000", "0.08"),
        ("567", "灯珠2835HSG", "1000", "0.05"),
    ]);
    let id = create_draft(&app, 129, &items_json).await;

    let order = get_order(&app, id).await;
    // 1000×0.15 + 1000×0.08 + 1000×0.05 = 150 + 80 + 50 = 280
    assert_eq!(order.amount_total, Decimal::from(280),
        "total should be 280, got {}", order.amount_total);
    assert_eq!(order.status, PurchaseOrderStatus::Draft);

    // Detail page: verify product codes appear
    let resp = app.get_htmx(&format!("/admin/purchase/orders/{id}")).await;
    assert!(resp.is_ok());
    assert!(resp.body_contains("3010134033"), "product 565 code");
    assert!(resp.body_contains("x1739581537"), "product 566 code");
    assert!(resp.body_contains("3010135078"), "product 567 code");
}

#[tokio::test]
async fn create_then_verify_supplier_on_detail() {
    let app = TestApp::new().await;
    let items_json = items_json(&[("565", "供应商验证", "10", "5.00")]);
    let id = create_draft(&app, 129, &items_json).await;

    let resp = app.get(&format!("/admin/purchase/orders/{id}")).await;
    assert!(resp.is_ok());
    // Supplier 129 = 开平市亿鑫光电科技有限公司
    assert!(resp.body_contains("亿鑫"),
        "detail page should contain supplier name");
}

#[tokio::test]
async fn create_then_verify_detail_shows_doc_number() {
    let app = TestApp::new().await;
    let items_json = items_json(&[("565", "单号验证", "1", "1")]);
    let id = create_draft(&app, 129, &items_json).await;

    let order = get_order(&app, id).await;
    let resp = app.get(&format!("/admin/purchase/orders/{id}")).await;
    assert!(resp.is_ok());
    assert!(resp.body_contains(&order.doc_number),
        "detail should show doc_number '{}'", order.doc_number);
}

// ════════════════════════════════════════════════════════════════════════════
//  Create → validation (realistic — handler accepts many inputs, errors are
//  only on genuine parse failures)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_invalid_date_returns_400() {
    let app = TestApp::new().await;
    let resp = app.post_htmx("/admin/purchase/orders/create",
        "supplier_id=129&order_date=not-a-date&items_json=%5B%5D").await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST,
        "bogus date should fail, got {}", resp.status);
}

#[tokio::test]
async fn create_malformed_json_returns_400() {
    let app = TestApp::new().await;
    let resp = app.post_htmx("/admin/purchase/orders/create",
        "supplier_id=129&order_date=2026-06-16&items_json=NOT_JSON").await;
    assert!(resp.status.is_client_error(),
        "malformed JSON should fail, got {}", resp.status);
}

#[tokio::test]
async fn create_with_empty_items_still_succeeds() {
    // NOTE: the handler currently accepts empty items_json="[]" and creates
    // a PO with zero line items. Documented as observed behavior — if this
    // should be rejected, add validation in the handler.
    let app = TestApp::new().await;
    let body = "supplier_id=129&order_date=2026-06-16&items_json=%5B%5D";
    let resp = app.post_htmx("/admin/purchase/orders/create", body).await;
    assert!(resp.is_ok(),
        "empty items currently succeeds (no validation), got {}", resp.status);
}

#[tokio::test]
async fn create_with_nonexistent_supplier_succeeds() {
    // NOTE: supplier existence is not validated during creation. Documented
    // as observed behavior.
    let app = TestApp::new().await;
    let items = items_json(&[("565", "x", "1", "1")]);
    let body = format!("supplier_id=999999&order_date=2026-06-16&items_json={items}");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(),
        "non-existent supplier currently succeeds, got {}", resp.status);
}

// ════════════════════════════════════════════════════════════════════════════
//  List & read pages
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_page_renders() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/orders").await;
    assert!(resp.is_ok() && resp.body_contains("<html") && resp.body_contains("采购订单"));
}

#[tokio::test]
async fn list_filters_and_pagination() {
    let app = TestApp::new().await;
    assert!(app.get_htmx("/admin/purchase/orders?status=1").await.is_ok());
    assert!(app.get_htmx("/admin/purchase/orders?supplier_id=129").await.is_ok());
    assert!(app.get_htmx("/admin/purchase/orders?page=2").await.is_ok());
    assert!(app.get_htmx("/admin/purchase/orders?keyword=PO-2026").await.is_ok());
}

#[tokio::test]
async fn detail_not_found_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/admin/purchase/orders/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn create_page_renders() {
    let app = TestApp::new().await;
    let full = app.get("/admin/purchase/orders/create").await;
    assert!(full.is_ok() && full.body_contains("<html"));
    assert!(full.body_contains("供应商") || full.body_contains("supplier"));
    let htmx = app.get_htmx("/admin/purchase/orders/create").await;
    assert!(htmx.is_ok() && !htmx.body_contains("<html"));
}

#[tokio::test]
async fn create_sub_endpoints() {
    let app = TestApp::new().await;
    assert!(app.get_htmx("/admin/purchase/orders/create/supplier-detail?supplier_id=129").await.is_ok());
    assert!(app.get_htmx("/admin/purchase/orders/products?name=2835").await.is_ok());
    assert!(app.get_htmx("/admin/purchase/orders/create/item-row?product_id=565").await.is_ok());
}

// ════════════════════════════════════════════════════════════════════════════
//  Submit → verify status transition in DB
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn submit_transitions_draft_to_confirmed() {
    let app = TestApp::new().await;
    let items = items_json(&[("565", "提交深测", "20", "50.00")]);
    let id = create_draft(&app, 129, &items).await;

    let before = get_order(&app, id).await;
    assert_eq!(before.status, PurchaseOrderStatus::Draft);

    let resp = app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    assert!(resp.is_ok(),
        "submit failed: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    let redirect = resp.hx_redirect().unwrap();
    assert!(redirect.contains(&format!("/admin/purchase/orders/{id}")));

    let after = get_order(&app, id).await;
    assert_eq!(after.status, PurchaseOrderStatus::Confirmed,
        "submit → Confirmed, got {:?}", after.status);
}

#[tokio::test]
async fn submit_already_confirmed_returns_error() {
    let app = TestApp::new().await;
    let items = items_json(&[("565", "重复提交深测", "5", "1")]);
    let id = create_draft(&app, 129, &items).await;

    // First submit
    app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    let after1 = get_order(&app, id).await;
    assert_eq!(after1.status, PurchaseOrderStatus::Confirmed);

    // Second submit should fail
    let resp2 = app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    assert!(resp2.status.is_client_error(),
        "double submit should fail, got {}", resp2.status);

    // Status must remain Confirmed
    let after2 = get_order(&app, id).await;
    assert_eq!(after2.status, PurchaseOrderStatus::Confirmed,
        "status must not change after failed re-submit");
}

// ════════════════════════════════════════════════════════════════════════════
//  Cancel → verify status transition in DB
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn cancel_draft_transitions_to_cancelled() {
    let app = TestApp::new().await;
    let items = items_json(&[("565", "取消深测", "1", "1")]);
    let id = create_draft(&app, 129, &items).await;
    assert_eq!(get_order(&app, id).await.status, PurchaseOrderStatus::Draft);

    app.post_htmx(&format!("/admin/purchase/orders/{id}/cancel"), "").await;
    let after = get_order(&app, id).await;
    assert_eq!(after.status, PurchaseOrderStatus::Cancelled,
        "cancel → Cancelled, got {:?}", after.status);
}

// ════════════════════════════════════════════════════════════════════════════
//  Edit
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn edit_page_shows_existing_items() {
    let app = TestApp::new().await;
    let items = items_json(&[("565", "编辑页-物料X", "50", "3.50")]);
    let id = create_draft(&app, 129, &items).await;

    let resp = app.get(&format!("/admin/purchase/orders/{id}/edit")).await;
    assert!(resp.is_ok(), "edit page: {}", resp.status);
    assert!(resp.body_contains("编辑页-物料X"),
        "edit page should show existing item description");
}

#[tokio::test]
async fn edit_confirmed_order_rejected() {
    let app = TestApp::new().await;
    let items = items_json(&[("565", "编辑拒绝深测", "1", "1")]);
    let id = create_draft(&app, 129, &items).await;

    app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    assert_eq!(get_order(&app, id).await.status, PurchaseOrderStatus::Confirmed);

    let resp = app.get(&format!("/admin/purchase/orders/{id}/edit")).await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn action_on_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.post_htmx("/admin/purchase/orders/999999/submit", "").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post_htmx("/admin/purchase/orders/999999/cancel", "").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  Full lifecycle: create → submit → verify all transitions
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn full_lifecycle_create_submit_verify() {
    let app = TestApp::new().await;
    let items = items_json(&[
        ("565", "全流程-物料A", "500", "2.00"),
        ("566", "全流程-物料B", "300", "5.00"),
    ]);
    let id = create_draft(&app, 129, &items).await;

    // 1. DB state after create
    let order = get_order(&app, id).await;
    assert_eq!(order.status, PurchaseOrderStatus::Draft);
    assert_eq!(order.supplier_id, 129);
    // 500×2 + 300×5 = 1000 + 1500 = 2500
    assert_eq!(order.amount_total, Decimal::from(2500));

    // 2. Detail page includes doc_number
    let detail = app.get(&format!("/admin/purchase/orders/{id}")).await;
    assert!(detail.body_contains(&order.doc_number),
        "detail should show doc_number '{}'", order.doc_number);

    // 3. Submit
    app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;

    // 4. DB state after submit
    let order2 = get_order(&app, id).await;
    assert_eq!(order2.status, PurchaseOrderStatus::Confirmed);
    assert_eq!(order2.amount_total, Decimal::from(2500));
    assert_eq!(order2.supplier_id, 129);
}
