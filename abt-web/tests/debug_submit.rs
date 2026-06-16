//! Debug test: capture the actual error from submit.

mod common;

use common::TestApp;

#[tokio::test]
async fn debug_submit_error() {
    let app = TestApp::new().await;

    // Create a fresh draft PO
    let items_json = urlencoding(
        r#"[{"product_id":"565","description":"测试物料","quantity":"10","unit_price":"1.50","item_delivery_date":null,"discount_pct":null,"tax_rate_id":null}]"#,
    );
    let body = format!("supplier_id=129&order_date=2026-06-16&items_json={items_json}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    eprintln!("Create status: {}, body: {}", resp.status, &resp.body[..200.min(resp.body.len())]);

    let redirect = resp.hx_redirect().unwrap();
    let id: i64 = redirect.trim_start_matches("/admin/purchase/orders/").parse().unwrap();
    eprintln!("Created order id: {id}");

    // Now submit
    let resp = app.post_htmx(&format!("/admin/purchase/orders/{id}/submit"), "").await;
    eprintln!("Submit status: {}", resp.status);
    eprintln!("Submit body: {}", &resp.body);
    eprintln!("Submit headers: {:?}", resp.headers);
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
