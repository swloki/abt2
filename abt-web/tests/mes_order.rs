//! 工单（Work Order）Handler 集成测试
//!
//! 覆盖：创建 / 字段验证 / 创建页渲染 / 来源单据搜索。
//! 工单操作（下达 / 关闭 / 取消 / 拆批 / 排程）已迁至 work-center 作业中心，
//! 旧路由 `/admin/mes/orders/{id}/*` 随详情页一并下线，对应状态流转测试已移除。

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::{
    mes::{
        enums::WorkOrderStatus,
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
//  页面渲染
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/orders/create").await.is_ok());
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
