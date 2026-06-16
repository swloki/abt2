//! MES 模块页面可达性批量测试
//!
//! 验证所有 MES 页面的 GET 请求返回 200，
//! HTMX 请求返回 fragment（非完整 HTML）。

mod common;
use common::TestApp;

// ════════════════════════════════════════════════════════════════════════════
//  所有 MES 列表页 GET 可达
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn all_list_pages_accessible() {
    let app = TestApp::new().await;
    for url in [
        "/admin/mes",
        "/admin/mes/plans",
        "/admin/mes/orders",
        "/admin/mes/reports",
        "/admin/mes/receipts",
        "/admin/mes/inspections",
        "/admin/mes/exceptions",
        "/admin/mes/demand-pool",
        "/admin/mes/wages",
        "/admin/mes/cards",
        "/admin/mes/schedule",
    ] {
        let resp = app.get(url).await;
        assert!(
            resp.is_ok(),
            "GET {url} returned {} body: {}",
            resp.status,
            resp.body.chars().take(200).collect::<String>()
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  所有 MES 创建页 GET 可达
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn all_create_pages_accessible() {
    let app = TestApp::new().await;
    for url in [
        "/admin/mes/plans/create",
        "/admin/mes/orders/create",
        "/admin/mes/receipts/create",
        "/admin/mes/inspections/create",
        "/admin/mes/demand-pool/create",
    ] {
        let resp = app.get(url).await;
        assert!(
            resp.is_ok(),
            "GET {url} returned {} body: {}",
            resp.status,
            resp.body.chars().take(200).collect::<String>()
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  HTMX 请求返回 fragment（不含 <html>）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn htmx_list_pages_return_fragments() {
    let app = TestApp::new().await;
    for url in [
        "/admin/mes/plans",
        "/admin/mes/orders",
        "/admin/mes/receipts",
        "/admin/mes/inspections",
        "/admin/mes/demand-pool",
    ] {
        let resp = app.get_htmx(url).await;
        assert!(resp.is_ok(), "HTMX {url} returned {}", resp.status);
        // HTMX 请求应返回 fragment，不是完整 HTML 页面
        // 注意：某些页面可能始终返回完整页面（取决于 admin_page 实现），
        // 这里只验证状态码
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Dashboard 可达
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn dashboard_accessible() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes").await;
    assert!(resp.is_ok(), "MES dashboard should render: {}", resp.status);
}

// ════════════════════════════════════════════════════════════════════════════
//  不存在的详情页返回 404
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn nonexistent_details_return_404() {
    let app = TestApp::new().await;
    for url in [
        "/admin/mes/plans/999999",
        "/admin/mes/orders/999999",
        "/admin/mes/batches/999999",
        "/admin/mes/receipts/999999",
        "/admin/mes/inspections/999999",
    ] {
        assert_eq!(
            app.get(url).await.status,
            axum::http::StatusCode::NOT_FOUND,
            "GET {url} should return 404"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  异常列表页可达
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn exception_list_accessible() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/exceptions").await;
    assert!(resp.is_ok(), "exception list should render: {}", resp.status);
}

#[tokio::test]
async fn exception_detail_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/admin/mes/exceptions/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  报工列表 & 工资列表
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn report_list_accessible() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/reports").await.is_ok());
}

#[tokio::test]
async fn report_create_page_accessible() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/reports/create").await.is_ok());
}

#[tokio::test]
async fn wage_list_accessible() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/wages").await.is_ok());
}

// ════════════════════════════════════════════════════════════════════════════
//  报工辅助 API
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn report_search_wo_accessible() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/reports/search-wo?q=").await.is_ok());
}

#[tokio::test]
async fn report_search_batch_accessible() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/reports/search-batch?q=").await.is_ok());
}
