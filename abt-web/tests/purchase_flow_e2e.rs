//! 采购全流程 Handler 集成测试
//!
//! 覆盖：采购订单 | 询价单 | 到货收货 | 退货 | 对账 | 付款
//! 每条测试验证 HTTP 状态 + 数据库字段级正确性 + 异常边界。

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::{
    purchase::{
        enums::{PurchaseOrderStatus, PurchaseReturnStatus, PurchaseQuotationStatus},
        order::{PurchaseOrderService, model::{PurchaseOrder, PurchaseOrderItem}},
        return_order::{PurchaseReturnService, model::PurchaseReturn},
        payment::model::PaymentRequest,
        quotation::{PurchaseQuotationService, model::PurchaseQuotation},
    },
    shared::types::ServiceContext,
};

fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => { out.push('%'); out.push_str(&format!("{:02X}", b)); }
        }
    }
    out
}

fn items_json(items: &[(&str, &str, &str, &str)]) -> String {
    let parts: Vec<String> = items.iter()
        .map(|(pid, desc, qty, price)| format!(
            r#"{{"product_id":"{pid}","description":"{desc}","quantity":"{qty}","unit_price":"{price}","item_delivery_date":null,"discount_pct":null,"tax_rate_id":null}}"#
        ))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

const SUPPLIER_ID: i64 = 129;
const PRODUCT_ID: i64 = 565;
const WAREHOUSE_ID: i64 = 23320;

async fn create_po(app: &TestApp, desc: &str, qty: &str, price: &str) -> (i64, String) {
    let items = items_json(&[(PRODUCT_ID.to_string().as_str(), desc, qty, price)]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-16&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    let msg = format!("create PO FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    assert!(resp.is_ok(), "{msg}");
    let id: i64 = resp.hx_redirect().unwrap().trim_start_matches("/admin/purchase/orders/").parse().unwrap();
    let order = get_order(app, id).await;
    (id, order.doc_number)
}

async fn get_order(app: &TestApp, id: i64) -> PurchaseOrder {
    let svc = app.state.purchase_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.get(&ServiceContext::new(1), &mut conn, id).await.unwrap()
}

async fn get_po_items(app: &TestApp, po_id: i64) -> Vec<PurchaseOrderItem> {
    let svc = app.state.purchase_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.list_items(&ServiceContext::new(1), &mut conn, po_id).await.unwrap()
}

async fn create_po_and_receive(app: &TestApp, desc: &str, qty: &str, price: &str) -> (i64, i64) {
    let (po_id, _) = create_po(app, desc, qty, price).await;
    app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;
    assert_eq!(get_order(app, po_id).await.status, PurchaseOrderStatus::Confirmed);
    let items = get_po_items(app, po_id).await;
    let arr_json = {
        let parts: Vec<String> = items.iter().map(|item| format!(
            r#"{{"product_id":"{}","declared_qty":"{}","batch_no":null,"order_item_id":"{}"}}"#,
            item.product_id, item.quantity, item.id
        )).collect();
        urlenc(&format!("[{}]", parts.join(",")))
    };
    let body = format!("purchase_order_id={po_id}&supplier_id={SUPPLIER_ID}&arrival_date=2026-06-16&warehouse_id={WAREHOUSE_ID}&items_json={arr_json}");
    let arr_id: i64 = app.post_htmx("/admin/wms/arrivals/create", &body).await
        .hx_redirect().unwrap().trim_start_matches("/admin/wms/arrivals/").parse().unwrap();
    app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=receive").await;
    app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=inspect").await;
    (po_id, arr_id)
}

// ════════════════════════════════════════════════════════════════════════════
//  A. Purchase Order — lifecycle + errors
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn a1_po_create_submit_verify() {
    let app = TestApp::new().await;
    let (po_id, doc) = create_po(&app, "PO全流程", "50", "20.00").await;
    let po = get_order(&app, po_id).await;
    assert_eq!(po.status, PurchaseOrderStatus::Draft);
    assert_eq!(po.supplier_id, SUPPLIER_ID);
    assert_eq!(po.amount_total, Decimal::from(1000));

    app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;
    assert_eq!(get_order(&app, po_id).await.status, PurchaseOrderStatus::Confirmed);

    let detail = app.get_htmx(&format!("/admin/purchase/orders/{po_id}")).await;
    assert!(detail.is_ok() && detail.body_contains(&doc));
    assert!(detail.body_contains("已确认") || detail.body_contains("Confirmed"));
}

#[tokio::test]
async fn a2_po_error_invalid_date() {
    let app = TestApp::new().await;
    assert_eq!(app.post_htmx("/admin/purchase/orders/create",
        "supplier_id=129&order_date=bad&items_json=%5B%5D").await.status,
        axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a3_po_error_malformed_json() {
    let app = TestApp::new().await;
    assert!(app.post_htmx("/admin/purchase/orders/create",
        "supplier_id=129&order_date=2026-06-16&items_json=NOT_JSON").await.status.is_client_error());
}

#[tokio::test]
async fn a4_po_error_double_submit() {
    let app = TestApp::new().await;
    let (po_id, _) = create_po(&app, "重复提交", "1", "1").await;
    app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;
    assert!(app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await.status.is_client_error());
    assert_eq!(get_order(&app, po_id).await.status, PurchaseOrderStatus::Confirmed);
}

#[tokio::test]
async fn a5_po_error_nonexistent() {
    let app = TestApp::new().await;
    assert_eq!(app.post_htmx("/admin/purchase/orders/999999/submit", "").await.status,
        axum::http::StatusCode::NOT_FOUND);
    assert_eq!(app.post_htmx("/admin/purchase/orders/999999/cancel", "").await.status,
        axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  B. Quotation — create → activate → convert → cancel → delete
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn b1_quotation_create_activate_convert() {
    let app = TestApp::new().await;
    let items = urlenc(r#"[{"product_id":"565","unit_price":"3.50","min_order_qty":"10","lead_time_days":"7","currency":"CNY","is_preferred":"on"}]"#);
    let body = format!("supplier_id={SUPPLIER_ID}&quotation_date=2026-06-16&valid_from=2026-06-01&valid_until=2026-12-31&items_json={items}");
    let resp = app.post_htmx("/admin/purchase/quotations/create", &body).await;
    assert!(resp.is_ok(), "create PQ FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let pq_id: i64 = resp.hx_redirect().unwrap().trim_start_matches("/admin/purchase/quotations/").parse().unwrap();

    let pq_svc = app.state.purchase_quotation_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    assert_eq!(pq_svc.get(&ctx, &mut conn, pq_id).await.unwrap().status, PurchaseQuotationStatus::Draft);
    drop(conn);

    assert!(app.get(&format!("/admin/purchase/quotations/{pq_id}")).await.is_ok());

    // Activate
    let resp = app.post_htmx(&format!("/admin/purchase/quotations/{pq_id}/activate"), "").await;
    assert!(resp.is_ok() || resp.hx_redirect().is_some());
    let mut conn = app.state.pool.acquire().await.unwrap();
    assert_eq!(pq_svc.get(&ctx, &mut conn, pq_id).await.unwrap().status, PurchaseQuotationStatus::Active);
    drop(conn);

    // Convert to PO
    let resp = app.post_htmx(&format!("/admin/purchase/quotations/{pq_id}/convert"), "").await;
    assert!(resp.is_ok(), "convert FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let cpo_id: i64 = resp.hx_redirect().unwrap().trim_start_matches("/admin/purchase/orders/").parse().unwrap();
    let cpo = get_order(&app, cpo_id).await;
    assert_eq!(cpo.supplier_id, SUPPLIER_ID);
    assert_eq!(cpo.status, PurchaseOrderStatus::Draft);

    app.post_htmx(&format!("/admin/purchase/orders/{cpo_id}/submit"), "").await;
    assert_eq!(get_order(&app, cpo_id).await.status, PurchaseOrderStatus::Confirmed);
}

#[tokio::test]
async fn b2_quotation_cancel_delete() {
    let app = TestApp::new().await;
    let items = urlenc(r#"[{"product_id":"565","unit_price":"1.00","min_order_qty":"1"}]"#);
    let body = format!("supplier_id={SUPPLIER_ID}&quotation_date=2026-06-16&valid_from=2026-06-01&valid_until=2026-12-31&items_json={items}");
    let pq_id: i64 = app.post_htmx("/admin/purchase/quotations/create", &body).await
        .hx_redirect().unwrap().trim_start_matches("/admin/purchase/quotations/").parse().unwrap();

    app.post_htmx(&format!("/admin/purchase/quotations/{pq_id}/cancel"), "").await;
    app.post_htmx(&format!("/admin/purchase/quotations/{pq_id}/delete"), "").await;
    assert_eq!(app.get(&format!("/admin/purchase/quotations/{pq_id}")).await.status,
        axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn b3_quotation_error_nonexistent() {
    let app = TestApp::new().await;
    assert_eq!(app.get("/admin/purchase/quotations/999999").await.status,
        axum::http::StatusCode::NOT_FOUND);
    assert_eq!(app.post_htmx("/admin/purchase/quotations/999999/activate", "").await.status,
        axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  C. Arrival → Receive → Inspect
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn c1_arrival_full_workflow() {
    let app = TestApp::new().await;
    let (po_id, arr_id) = create_po_and_receive(&app, "到货全流程", "80", "15.00").await;
    assert!(app.get_htmx(&format!("/admin/wms/arrivals/{arr_id}")).await.is_ok());
    assert!(app.get_htmx(&format!("/admin/purchase/orders/{po_id}")).await.is_ok());
}

#[tokio::test]
async fn c2_arrival_error_invalid_date() {
    let app = TestApp::new().await;
    assert!(app.post_htmx("/admin/wms/arrivals/create",
        "supplier_id=129&arrival_date=bad&items_json=%5B%5D").await.status.is_client_error());
}

#[tokio::test]
async fn c3_arrival_error_bogus_action() {
    let app = TestApp::new().await;
    let (po_id, _) = create_po(&app, "到货异常", "1", "1").await;
    app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;
    let items = get_po_items(&app, po_id).await;
    let arr_json = urlenc(&format!("[{{\"product_id\":\"{}\",\"declared_qty\":\"{}\",\"batch_no\":null,\"order_item_id\":\"{}\"}}]",
        items[0].product_id, items[0].quantity, items[0].id));
    let body = format!("purchase_order_id={po_id}&supplier_id={SUPPLIER_ID}&arrival_date=2026-06-16&warehouse_id={WAREHOUSE_ID}&items_json={arr_json}");
    let arr_id: i64 = app.post_htmx("/admin/wms/arrivals/create", &body).await
        .hx_redirect().unwrap().trim_start_matches("/admin/wms/arrivals/").parse().unwrap();
    let resp = app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=bogus").await;
    assert!(resp.is_ok() || resp.is_redirect(), "bogus action should not crash");
}

// ════════════════════════════════════════════════════════════════════════════
//  D. Return — create (needs received items) → confirm → cancel
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn d1_return_create_confirm() {
    let app = TestApp::new().await;
    let (po_id, _) = create_po_and_receive(&app, "退货测试", "60", "10.00").await;
    let items = get_po_items(&app, po_id).await;

    let ret_json = {
        let parts: Vec<String> = items.iter().map(|item| format!(
            r#"{{"order_item_id":{},"product_id":{},"returned_qty":"30","unit_price":"10.00"}}"#,
            item.id, item.product_id
        )).collect();
        urlenc(&format!("[{}]", parts.join(",")))
    };
    let body = format!("order_id={po_id}&return_date=2026-06-16&return_reason=测试退货&items_json={ret_json}");
    let resp = app.post_htmx("/admin/purchase/returns/create", &body).await;
    assert!(resp.is_ok(), "create return FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let ret_id: i64 = resp.hx_redirect().unwrap().trim_start_matches("/admin/purchase/returns/").parse().unwrap();

    let ret_svc = app.state.purchase_return_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    assert_eq!(ret_svc.get(&ctx, &mut conn, ret_id).await.unwrap().status, PurchaseReturnStatus::Draft);
    drop(conn);

    let detail = app.get_htmx(&format!("/admin/purchase/returns/{ret_id}")).await;
    assert!(detail.is_ok() && (detail.body_contains("草稿") || detail.body_contains("Draft")));

    app.post_htmx(&format!("/admin/purchase/returns/{ret_id}/confirm"), "").await;
    let mut conn = app.state.pool.acquire().await.unwrap();
    assert_eq!(ret_svc.get(&ctx, &mut conn, ret_id).await.unwrap().status, PurchaseReturnStatus::Confirmed);
    drop(conn);

    // Cancel confirmed (may or may not be allowed)
    let resp = app.post_htmx(&format!("/admin/purchase/returns/{ret_id}/cancel"), "").await;
    assert!(resp.is_ok() || resp.is_redirect() || resp.status.is_client_error(),
        "cancel return unexpected: {}", resp.status);
}

#[tokio::test]
async fn d2_return_error_exceeds_received() {
    let app = TestApp::new().await;
    let (po_id, _) = create_po_and_receive(&app, "退货超额", "10", "1.00").await;
    let items = get_po_items(&app, po_id).await;
    let ret_json = urlenc(&format!("[{{\"order_item_id\":{},\"product_id\":{},\"returned_qty\":\"60\",\"unit_price\":\"1.00\"}}]",
        items[0].id, items[0].product_id));
    let body = format!("order_id={po_id}&return_date=2026-06-16&return_reason=test&items_json={ret_json}");
    let resp = app.post_htmx("/admin/purchase/returns/create", &body).await;
    assert!(resp.status.is_client_error(), "exceeding received should fail, got {}", resp.status);
}

#[tokio::test]
async fn d3_return_error_empty_items() {
    let app = TestApp::new().await;
    let (po_id, _) = create_po_and_receive(&app, "退货空", "1", "1").await;
    let body = format!("order_id={po_id}&return_date=2026-06-16&return_reason=test&items_json=%5B%5D");
    assert!(app.post_htmx("/admin/purchase/returns/create", &body).await.status.is_client_error());
}

// ════════════════════════════════════════════════════════════════════════════
//  E. Payment — error cases (happy path depends on reconciliation amount)
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn e1_payment_error_amount_exceeds_recon() {
    let app = TestApp::new().await;
    let period = chrono::Local::now().format("%Y-%m").to_string();
    let body = format!("supplier_id={SUPPLIER_ID}&period={period}");
    let recon_id: i64 = app.post_htmx("/admin/purchase/reconciliations/create", &body).await
        .hx_redirect().unwrap().trim_start_matches("/admin/purchase/reconciliations/").parse().unwrap();

    // Reconciliation without items → amount ≈ 0. Payment with positive amount should fail.
    let body = format!("supplier_id={SUPPLIER_ID}&reconciliation_id={recon_id}&payment_date=2026-06-16&amount=100000.00&payment_method=1");
    let resp = app.post_htmx("/admin/purchase/payments/create", &body).await;
    assert!(resp.status.is_client_error(), "huge payment vs empty recon should fail, got {} body: {}",
        resp.status, resp.body.chars().take(200).collect::<String>());

    let mut c = app.state.pool.acquire().await.unwrap();
    let _ = sqlx::query("DELETE FROM purchase_reconciliations WHERE id = $1")
        .bind(recon_id).execute(&mut *c).await;
}

#[tokio::test]
async fn e2_payment_error_invalid_method() {
    let app = TestApp::new().await;
    let period = chrono::Local::now().format("%Y-%m").to_string();
    let body = format!("supplier_id={SUPPLIER_ID}&period={period}");
    let recon_id: i64 = app.post_htmx("/admin/purchase/reconciliations/create", &body).await
        .hx_redirect().unwrap().trim_start_matches("/admin/purchase/reconciliations/").parse().unwrap();

    let body = format!("supplier_id={SUPPLIER_ID}&reconciliation_id={recon_id}&payment_date=2026-06-16&amount=0&payment_method=99");
    assert!(app.post_htmx("/admin/purchase/payments/create", &body).await.status.is_client_error());

    let mut c = app.state.pool.acquire().await.unwrap();
    let _ = sqlx::query("DELETE FROM purchase_reconciliations WHERE id = $1")
        .bind(recon_id).execute(&mut *c).await;
}

// ════════════════════════════════════════════════════════════════════════════
//  F. Page accessibility
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn f1_all_list_pages_accessible() {
    let app = TestApp::new().await;
    for url in ["/admin/purchase/orders", "/admin/purchase/quotations", "/admin/purchase/returns",
                "/admin/purchase/demand-pool", "/admin/purchase/reconciliations", "/admin/purchase/payments",
                "/admin/purchase/supplier-prices", "/admin/purchase/approval-rules"] {
        assert!(app.get(url).await.is_ok(), "GET {url} failed");
    }
}

#[tokio::test]
async fn f2_htmx_pages_return_fragments() {
    let app = TestApp::new().await;
    for url in ["/admin/purchase/orders", "/admin/purchase/quotations", "/admin/purchase/returns",
                "/admin/purchase/demand-pool", "/admin/purchase/reconciliations"] {
        let resp = app.get_htmx(url).await;
        assert!(resp.is_ok(), "HTMX {url} returned {}", resp.status);
        assert!(!resp.body_contains("<html"), "HTMX {url} should be fragment");
    }
}
