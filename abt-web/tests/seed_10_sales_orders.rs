//! 生成 10 张销售订单（自制产品 PRODUCT_MADE，需生产）作为测试数据。
//!
//! SO create + confirm 后，自制产品行进入生产需求池（/admin/mes/demand-pool）。
//! 后续可在工作中心「创建工单」把需求转化为工单，验证批次 tab drawer 矩阵 + 齐套。

mod common;
use common::TestApp;

const CUSTOMER_ID: i64 = 135;
const CONTACT_ID: i64 = 135;
/// 自制产品（acquire_channel=SelfProduced，有 BOM bom_id=1000157，倒冲模式）
const PRODUCT_MADE: i64 = 8665;

fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 销售订单明细 JSON（URL 编码）：[{product_id, quantity, unit_price}]
fn so_items(items: &[(&str, &str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty, price)| {
            format!(r#"{{"product_id":"{pid}","quantity":"{qty}","unit_price":"{price}"}}"#)
        })
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

/// 从 HX-Redirect 提取末尾 id（/admin/orders/123 → 123）
fn redirect_id(resp: &common::TestResponse) -> i64 {
    let loc = resp.hx_redirect().unwrap_or_else(|| {
        resp.headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    });
    loc.rsplit('/').next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

#[tokio::test]
async fn seed_10_sales_orders() {
    let app = TestApp::new().await;
    for i in 1..=10 {
        let qty = (50 + i * 10).to_string(); // 60, 70, ..., 150
        let so_body = format!(
            "customer_id={CUSTOMER_ID}&contact_id={CONTACT_ID}&items_json={}",
            so_items(&[(&PRODUCT_MADE.to_string(), &qty, "1.00")])
        );
        let resp = app.post_htmx("/admin/orders/create", &so_body).await;
        let so_id = redirect_id(&resp);
        assert!(
            so_id > 0,
            "SO #{} 创建失败: status={} body={}",
            i,
            resp.status,
            resp.body.chars().take(200).collect::<String>()
        );
        let conf = app
            .post_htmx(&format!("/admin/orders/{so_id}/confirm"), "")
            .await;
        println!(
            "SO #{i}: id={so_id} qty={qty} confirm_status={}",
            conf.status
        );
    }
    println!("✅ 10 张销售订单（自制产品 {PRODUCT_MADE}）已创建并确认，进需求池");
}
