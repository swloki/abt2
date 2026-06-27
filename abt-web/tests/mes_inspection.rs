//! 质检（Production Inspection）Handler 集成测试
//!
//! 覆盖：创建/记录结果/首检巡检完工检/列表详情/异常输入

mod common;
use common::TestApp;

use abt_core::{
    mes::{
        enums::InspectionResultType,
        production_inspection::{ProductionInspection, ProductionInspectionService},
        work_order::{model::WorkOrderFilter, WorkOrderService},
    },
    shared::types::ServiceContext,
};

const PRODUCT_ID: i64 = 565;

async fn create_wo(app: &TestApp) -> i64 {
    let body = format!(
        "product_id={PRODUCT_ID}&planned_qty=10&scheduled_start=2026-06-20&scheduled_end=2026-07-20"
    );
    app.post_htmx("/admin/mes/orders/create", &body).await;
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

/// 创建检验单并返回 inspection_id
async fn create_inspection(app: &TestApp, wo_id: i64, insp_type: i16) -> i64 {
    let body = format!(
        "work_order_id={wo_id}&product_id={PRODUCT_ID}&inspection_type={insp_type}&sample_qty=10&inspection_date=2026-06-16"
    );
    let resp = app.post_htmx("/admin/mes/inspections/create", &body).await;
    assert!(resp.is_ok(), "create inspection FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    // 从 Service 查最新
    let svc = app.state.production_inspection_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list_inspections(
            &ServiceContext::new(1),
            &mut conn,
            abt_core::mes::production_inspection::InspectionListFilter {
                keyword: None,
                inspection_type: None,
            },
            1,
            1,
        )
        .await
        .unwrap();
    result.items.first().unwrap().id
}

async fn get_inspection(app: &TestApp, id: i64) -> ProductionInspection {
    let svc = app.state.production_inspection_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.find_by_id(&ServiceContext::new(1), &mut conn, id).await.unwrap()
}

// ════════════════════════════════════════════════════════════════════════════
//  创建
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_first_article_inspection() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 1).await; // FirstArticle
    let insp = get_inspection(&app, insp_id).await;
    assert_eq!(insp.work_order_id, wo_id);
    assert_eq!(insp.product_id, PRODUCT_ID);
    assert_eq!(insp.inspection_type.as_i16(), 1);
    assert!(!insp.doc_number.is_empty());
}

#[tokio::test]
async fn create_in_process_inspection() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 2).await; // InProcess
    assert_eq!(get_inspection(&app, insp_id).await.inspection_type.as_i16(), 2);
}

#[tokio::test]
async fn create_final_inspection() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 3).await; // Final
    assert_eq!(get_inspection(&app, insp_id).await.inspection_type.as_i16(), 3);
}

#[tokio::test]
async fn create_with_disposition_and_remark() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let body = format!(
        "work_order_id={wo_id}&product_id={PRODUCT_ID}&inspection_type=1&sample_qty=5&inspection_date=2026-06-16&disposition=返修&remark=测试备注"
    );
    let resp = app.post_htmx("/admin/mes/inspections/create", &body).await;
    assert!(resp.is_ok(), "create with disposition should succeed: {}", resp.status);
}

#[tokio::test]
async fn create_invalid_type_returns_400() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/inspections/create",
            "work_order_id=1&product_id=565&inspection_type=99&sample_qty=5&inspection_date=2026-06-16",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

// ════════════════════════════════════════════════════════════════════════════
//  记录结果
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn record_result_pass() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 1).await;

    app.post_htmx(&format!("/admin/mes/inspections/{insp_id}/record-result"), "result=1").await;
    assert_eq!(
        get_inspection(&app, insp_id).await.result,
        InspectionResultType::Pass
    );
}

#[tokio::test]
async fn record_result_fail() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 1).await;

    app.post_htmx(&format!("/admin/mes/inspections/{insp_id}/record-result"), "result=2").await;
    assert_eq!(
        get_inspection(&app, insp_id).await.result,
        InspectionResultType::Fail
    );
}

#[tokio::test]
async fn record_result_conditional() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 1).await;

    app.post_htmx(&format!("/admin/mes/inspections/{insp_id}/record-result"), "result=3").await;
    assert_eq!(
        get_inspection(&app, insp_id).await.result,
        InspectionResultType::Conditional
    );
}

#[tokio::test]
async fn record_result_invalid_returns_400() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 1).await;

    let resp = app.post_htmx(&format!("/admin/mes/inspections/{insp_id}/record-result"), "result=99").await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

// ════════════════════════════════════════════════════════════════════════════
//  不存在 / 404
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn detail_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.get("/admin/mes/inspections/999999").await.status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn record_result_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(app.post_htmx("/admin/mes/inspections/999999/record-result", "result=1").await.status, axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  列表 & 详情页
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/inspections").await.is_ok());
}

#[tokio::test]
async fn create_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/inspections/create").await.is_ok());
}

#[tokio::test]
async fn detail_page_shows_doc_number() {
    let app = TestApp::new().await;
    let wo_id = create_wo(&app).await;
    let insp_id = create_inspection(&app, wo_id, 1).await;
    let insp = get_inspection(&app, insp_id).await;
    let detail = app.get_htmx(&format!("/admin/mes/inspections/{insp_id}")).await;
    assert!(detail.is_ok());
    assert!(detail.body_contains(&insp.doc_number));
}
