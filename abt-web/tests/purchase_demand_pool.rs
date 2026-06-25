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

// ── issue #72: 从订单详情带 order_id 跳转 → 强制 detail 视图 + 订单筛选 chip ──

#[tokio::test]
async fn demand_pool_list_with_order_id_filter() {
    // order_id 存在时应渲染「订单 #X」筛选 chip（强制 detail 视图）
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/demand-pool?order_id=128").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(
        resp.body_contains("订单 #128"),
        "order_id 存在时应显示「订单 #128」筛选 chip"
    );
}

#[tokio::test]
async fn demand_pool_list_without_order_id_no_chip() {
    // 无 order_id 时不应渲染订单筛选 chip（chip 的清除链接 title）
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/demand-pool").await;
    assert!(resp.is_ok());
    assert!(
        !resp.body_contains("清除订单筛选"),
        "无 order_id 时不应显示订单筛选 chip"
    );
}

#[tokio::test]
async fn demand_pool_order_id_htmx_fragment() {
    // HTMX 请求带 order_id → 返回片段（无 <html>）且含订单 chip
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/purchase/demand-pool?order_id=128").await;
    assert!(resp.is_ok());
    assert!(!resp.body_contains("<html"), "HTMX 应返回片段");
    assert!(resp.body_contains("订单 #128"));
}

// ── 排序下拉：物料汇总视图 sort 参数渲染 ──

#[tokio::test]
async fn demand_pool_material_sort_newest() {
    // sort=newest 渲染物料汇总排序下拉，含「新订单优先」选项
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/demand-pool?view=material&sort=newest").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(
        resp.body_contains("新订单优先"),
        "sort=newest 时应渲染「新订单优先」选项"
    );
}

#[tokio::test]
async fn demand_pool_material_sort_due() {
    // sort=due 渲染物料汇总排序下拉，含「到期日优先」选项
    let app = TestApp::new().await;
    let resp = app.get("/admin/purchase/demand-pool?view=material&sort=due").await;
    assert!(resp.is_ok(), "status {}", resp.status);
    assert!(
        resp.body_contains("到期日优先"),
        "sort=due 时应渲染「到期日优先」选项"
    );
}
