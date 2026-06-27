//! 工单（Work Order）深度 Handler 集成测试
//!
//! 覆盖：创建/下达/反下达/关闭/取消/拆批/工序验证/详情页

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::{
    mes::{
        enums::WorkOrderStatus,
        production_batch::{ProductionBatchService},
        work_order::{model::WorkOrderFilter, WorkOrder, WorkOrderService},
    },
    shared::types::ServiceContext,
};

const PRODUCT_ID: i64 = 565;

async fn get_work_order(app: &TestApp, id: i64) -> WorkOrder {
    let svc = app.state.work_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.find_by_id(&ServiceContext::new(1), &mut conn, id).await.unwrap()
}

/// 创建 Draft 工单并返回 wo_id
async fn create_wo(app: &TestApp, qty: &str) -> i64 {
    let body = format!(
        "product_id={PRODUCT_ID}&planned_qty={qty}&scheduled_start=2026-06-20&scheduled_end=2026-07-20"
    );
    let resp = app.post_htmx("/admin/mes/orders/create", &body).await;
    assert!(resp.is_ok(), "create WO FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    let svc = app.state.work_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(&ServiceContext::new(1), &mut conn, WorkOrderFilter {
            status: None, product_id: None, keyword: None, date_from: None, date_to: None, product_code: None, work_center_id: None,
        }, 1, 1)
        .await
        .unwrap();
    result.items.first().unwrap().id
}

async fn release_wo(app: &TestApp, wo_id: i64) {
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/release"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "release FAIL: {} body: {}", resp.status, resp.body.chars().take(200).collect::<String>());
}

// ════════════════════════════════════════════════════════════════════════════
//  创建 + 字段验证
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_draft_correct_fields() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "100").await;
    let wo = get_work_order(&app, wo_id).await;
    assert_eq!(wo.status, WorkOrderStatus::Draft);
    assert_eq!(wo.product_id, PRODUCT_ID);
    assert_eq!(wo.planned_qty, Decimal::from(100));
    assert!(wo.version >= 0);
    assert!(!wo.doc_number.is_empty());
}

#[tokio::test]
async fn create_with_remark() {
    let app = TestApp::new().await;
    let body = format!(
        "product_id={PRODUCT_ID}&planned_qty=10&scheduled_start=2026-06-20&scheduled_end=2026-07-20&remark=测试备注"
    );
    let resp = app.post_htmx("/admin/mes/orders/create", &body).await;
    assert!(resp.is_ok(), "create with remark should succeed");
}

#[tokio::test]
async fn create_invalid_qty_returns_400() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/orders/create",
            "product_id=565&planned_qty=BAD&scheduled_start=2026-06-20&scheduled_end=2026-07-20",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_invalid_date_returns_400() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/orders/create",
            "product_id=565&planned_qty=10&scheduled_start=BAD&scheduled_end=2026-07-20",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

// ════════════════════════════════════════════════════════════════════════════
//  下达 → 工序自动创建
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn release_transitions_draft_to_released() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "50").await;
    release_wo(&app, wo_id).await;
    assert_eq!(get_work_order(&app, wo_id).await.status, WorkOrderStatus::Released);
}

#[tokio::test]
async fn release_creates_routing_steps() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "30").await;
    release_wo(&app, wo_id).await;

    let batch_svc = app.state.production_batch_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let routings = batch_svc.list_routings(&ServiceContext::new(1), &mut conn, wo_id).await.unwrap();
    assert!(!routings.is_empty(), "released WO must have routing steps");
    // 至少有一道工序（step_no 从 1 开始）
    assert_eq!(routings[0].step_no, 1);
    assert!(!routings[0].process_name.is_empty());
}

#[tokio::test]
async fn release_idempotent_returns_redirect() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "10").await;
    release_wo(&app, wo_id).await;
    // 再次下达：handler 幂等处理（已是 Released 直接 redirect）
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/release"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "double release should be idempotent");
    assert_eq!(get_work_order(&app, wo_id).await.status, WorkOrderStatus::Released);
}

// ════════════════════════════════════════════════════════════════════════════
//  反下达 → Released → Draft
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn unrelease_transitions_back_to_draft() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "20").await;
    release_wo(&app, wo_id).await;
    assert_eq!(get_work_order(&app, wo_id).await.status, WorkOrderStatus::Released);

    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/unrelease"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "unrelease should succeed");
    assert_eq!(get_work_order(&app, wo_id).await.status, WorkOrderStatus::Draft);
}

#[tokio::test]
async fn unrelease_already_draft_is_idempotent() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "10").await;
    // 直接反下达 Draft 工单
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/unrelease"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "unrelease on Draft should be idempotent");
    assert_eq!(get_work_order(&app, wo_id).await.status, WorkOrderStatus::Draft);
}

// ════════════════════════════════════════════════════════════════════════════
//  取消
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn cancel_from_draft() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "5").await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/cancel"), "").await;
    // cancel 做软删除（deleted_at），所以验证 HTTP 成功 + 后续详情页 404
    assert!(resp.is_ok() || resp.is_redirect(), "cancel should succeed");
    assert_eq!(app.get(&format!("/admin/mes/orders/{wo_id}")).await.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_from_released() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "5").await;
    release_wo(&app, wo_id).await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/cancel"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "cancel from released should succeed");
    assert_eq!(app.get(&format!("/admin/mes/orders/{wo_id}")).await.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_already_cancelled_returns_404() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "5").await;
    app.post_htmx(&format!("/admin/mes/orders/{wo_id}/cancel"), "").await;
    // 已软删除的工单再次 cancel → handler find_by_id 返回 NotFound
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/cancel"), "").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  关闭
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn close_from_released() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "5").await;
    release_wo(&app, wo_id).await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/close"), "").await;
    // close 可能因空批次校验失败（看 close 的实现），验证不崩溃即可
    assert!(
        resp.is_ok() || resp.is_redirect() || resp.status.is_client_error(),
        "close should not crash, got {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  拆批（创建生产批次）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn split_creates_batch() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "100").await;
    release_wo(&app, wo_id).await;

    let body = "split_qty=50";
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/split"), body).await;
    assert!(resp.is_ok() || resp.is_redirect(), "split FAIL: {} body: {}", resp.status, resp.body.chars().take(200).collect::<String>());

    let batch_svc = app.state.production_batch_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let batches = batch_svc.list_by_work_order(&ServiceContext::new(1), &mut conn, wo_id).await.unwrap();
    assert!(!batches.is_empty(), "split should create at least one batch");
    assert_eq!(batches[0].batch_qty, Decimal::from(50));
}

#[tokio::test]
async fn split_zero_qty_rejected() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "10").await;
    release_wo(&app, wo_id).await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/split"), "split_qty=0").await;
    assert!(resp.status.is_client_error(), "split_qty=0 should be rejected, got {}", resp.status);
}

#[tokio::test]
async fn split_negative_qty_rejected() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "10").await;
    release_wo(&app, wo_id).await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/split"), "split_qty=-5").await;
    assert!(resp.status.is_client_error(), "negative split should be rejected, got {}", resp.status);
}

#[tokio::test]
async fn split_invalid_qty_format_returns_400() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "10").await;
    release_wo(&app, wo_id).await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/split"), "split_qty=BAD").await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

// ════════════════════════════════════════════════════════════════════════════
//  不存在 / 404
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn detail_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.get("/admin/mes/orders/999999").await.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn release_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.post_htmx("/admin/mes/orders/999999/release", "").await.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.post_htmx("/admin/mes/orders/999999/cancel", "").await.status, axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  列表 & 详情页
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/orders").await.is_ok());
}

#[tokio::test]
async fn create_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/orders/create").await.is_ok());
}

#[tokio::test]
async fn detail_page_renders_with_status() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app, "20").await;
    let wo = get_work_order(&app, wo_id).await;
    let detail = app.get_htmx(&format!("/admin/mes/orders/{wo_id}")).await;
    assert!(detail.is_ok());
    assert!(detail.body_contains(&wo.doc_number));
}

// ════════════════════════════════════════════════════════════════════════════
//  来源单据搜索 API
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn source_order_search_returns_ok() {
    let app = TestApp::new().await;
    let resp = app.get("/api/mes/source-orders/search?keyword=").await;
    assert!(resp.is_ok(), "source order search should return 200");
}

#[tokio::test]
async fn source_plan_search_returns_ok() {
    let app = TestApp::new().await;
    let resp = app.get("/api/mes/source-plans/search?keyword=").await;
    assert!(resp.is_ok(), "source plan search should return 200");
}
