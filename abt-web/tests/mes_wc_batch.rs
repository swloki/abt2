//! MES 工作中心「批次」tab handler 集成测试
//!
//! 覆盖批次 tab 端点（work-center batch 维度，Issue #121）：
//! - 批次列表渲染 + 状态筛选
//! - batch drawer / 各操作端点路由（404 守卫）
//!
//! 注：批次状态转换的底层逻辑由 mes_batch.rs 覆盖（同 service），此处聚焦
//! work-center 新端点的可达性与渲染，不依赖易耦合的工单 fixture。

mod common;
use common::TestApp;

/// 批次 tab 列表端点可达 + 渲染批次表（表头或空态）
#[tokio::test]
async fn wc_batch_tab_list_renders() {
    let app = TestApp::new().await;
    let resp = app
        .get("/admin/mes/work-center/demand?view=batches")
        .await;
    assert!(
        resp.is_ok(),
        "batch tab list FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    assert!(
        resp.body.contains("流转卡") || resp.body.contains("暂无批次"),
        "batch table header missing, body: {}",
        resp.body.chars().take(300).collect::<String>()
    );
}

/// 各批次状态筛选均不报错
#[tokio::test]
async fn wc_batch_tab_status_filter_ok() {
    let app = TestApp::new().await;
    for status in [
        "Pending", "InProgress", "Suspended", "PendingReceipt", "Completed", "Cancelled",
    ] {
        let resp = app
            .get(&format!(
                "/admin/mes/work-center/demand?view=batches&batch_status={status}"
            ))
            .await;
        assert!(
            resp.is_ok(),
            "status={status} filter FAIL: {}",
            resp.status
        );
    }
}

/// batch drawer 不存在批次 → 404（路由 + find_by_id 守卫）
#[tokio::test]
async fn wc_batch_drawer_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .get("/admin/mes/work-center/batches/99999999/drawer")
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// batch suspend 不存在批次 → 404
#[tokio::test]
async fn wc_batch_suspend_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/work-center/batches/99999999/suspend",
            "reason=test",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// batch advance 不存在批次 → 404
#[tokio::test]
async fn wc_batch_advance_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx("/admin/mes/work-center/batches/99999999/advance", "")
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// batch start 不存在批次 → 404
#[tokio::test]
async fn wc_batch_start_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx("/admin/mes/work-center/batches/99999999/start", "")
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// batch requisition 不存在批次 → 404
#[tokio::test]
async fn wc_batch_requisition_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/work-center/batches/99999999/requisition",
            "routing_id=1",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// batch receipt 不存在批次 → 404
#[tokio::test]
async fn wc_batch_receipt_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/work-center/batches/99999999/receipt",
            "warehouse_id=1&received_qty=1&receipt_date=2026-06-28",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

/// batch shortage 不存在批次 → 404（#124 工序级齐套展开端点）
#[tokio::test]
async fn wc_batch_shortage_nonexistent_404() {
    let app = TestApp::new().await;
    let resp = app
        .get("/admin/mes/work-center/batches/99999999/routings/1/shortage")
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}
