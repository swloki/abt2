//! 需求池 Handler 集成测试
//!
//! 覆盖：列表/物料聚合查询/创建计划页面/需求行查询

mod common;
use common::TestApp;

// ════════════════════════════════════════════════════════════════════════════
//  列表 & 查询
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_page_renders() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/demand-pool").await;
    assert!(resp.is_ok(), "demand pool list should render: {} body: {}", resp.status, resp.body.chars().take(200).collect::<String>());
}

#[tokio::test]
async fn list_htmx_returns_fragment() {
    let app = TestApp::new().await;
    let resp = app.get_htmx("/admin/mes/demand-pool").await;
    assert!(resp.is_ok(), "HTMX demand pool returned {}", resp.status);
}

#[tokio::test]
async fn demand_rows_endpoint_returns_ok() {
    let app = TestApp::new().await;
    // demand-rows 需要 product_id 参数
    let resp = app.get("/admin/mes/demand-pool/demand-rows?product_id=565").await;
    assert!(resp.is_ok(), "demand-rows endpoint should return 200, got {}", resp.status);
}
#[tokio::test]
async fn list_with_product_filter() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/demand-pool?product_id=565").await;
    assert!(resp.is_ok(), "demand pool with product filter should render");
}

// ════════════════════════════════════════════════════════════════════════════
//  创建计划页面（需求 → 计划）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_page_renders_without_params() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/demand-pool/create").await;
    assert!(resp.is_ok(), "demand pool create page should render without params");
}

#[tokio::test]
async fn create_page_with_product_id() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/demand-pool/create?product_id=565&product_name=TestProduct").await;
    assert!(resp.is_ok(), "demand pool create page with product should render");
}

#[tokio::test]
async fn create_page_with_demand_ids() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/demand-pool/create?product_id=565&demand_ids=1,2,3").await;
    assert!(resp.is_ok(), "demand pool create page with demand_ids should render");
}

// ════════════════════════════════════════════════════════════════════════════
//  从需求创建计划（需要真实需求数据，验证不崩溃）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_plan_with_empty_demands_does_not_crash() {
    let app = TestApp::new().await;
    // 空需求列表创建计划应返回错误但不崩溃
    let body = "plan_type=2&plan_date=2026-06-16&demand_ids=&items_json=%5B%5D";
    let resp = app.post_htmx("/admin/mes/demand-pool/create", body).await;
    assert!(
        resp.is_ok() || resp.status.is_client_error(),
        "empty demands create should not crash, got {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}
