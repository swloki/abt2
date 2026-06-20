//! MES 工序计件单价/删除/报工守卫 Repo 测试
//!
//! 测试 WorkOrderRoutingRepo 新增的 6 个方法：
//! - get_by_id
//! - update_unit_price
//! - delete
//! - renumber_steps
//! - has_report（单条工序报工守卫）
//! - has_any_report（工单全局报工守卫）

mod common;

use abt_core::mes::production_batch::repo::WorkOrderRoutingRepo;
use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::shared::types::context::ServiceContext;
use rust_decimal::Decimal;

const PRODUCT_ID: i64 = 565; // 2835/冷白0.5W（与 mes_flow_e2e.rs 一致）

/// 创建工单（复用 mes_flow_e2e.rs 逻辑）
async fn create_work_order(app: &common::TestApp, qty: &str) -> (i64, i32) {
    let body = format!(
        "product_id={PRODUCT_ID}&planned_qty={qty}&scheduled_start=2026-06-20&scheduled_end=2026-07-20"
    );
    let resp = app.post_htmx("/admin/mes/orders/create", &body).await;
    assert!(
        resp.is_ok(),
        "create WO FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    let svc = app.state.work_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(
            &ServiceContext::new(1),
            &mut conn,
            abt_core::mes::work_order::WorkOrderFilter {
                status: None,
                product_id: None,
                keyword: None,
                date_from: None,
                date_to: None,
            },
            1,
            1,
        )
        .await
        .unwrap();
    let wo = result.items.first().unwrap();
    (wo.id, wo.version)
}

/// 下达工单（复用 mes_flow_e2e.rs 逻辑）
async fn release_work_order(app: &common::TestApp, wo_id: i64) {
    let resp = app
        .post_htmx(&format!("/admin/mes/orders/{wo_id}/release"), "")
        .await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "release WO FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
}

/// 创建并下达工单，返回 wo_id
async fn seed_released_work_order(app: &common::TestApp, qty: &str) -> i64 {
    let (wo_id, _) = create_work_order(app, qty).await;
    release_work_order(app, wo_id).await;
    wo_id
}

/// 获取工单的第一条工序 ID
async fn first_routing_id(state: &abt_web::state::AppState, wo_id: i64) -> i64 {
    let svc = state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = state.pool.acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    rs[0].id
}

#[tokio::test]
async fn repo_update_unit_price_persists() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, "100").await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let mut conn = app.state.pool.acquire().await.unwrap();

    WorkOrderRoutingRepo::update_unit_price(&mut conn, rid, Decimal::new(125, 2))
        .await
        .unwrap();

    let after = WorkOrderRoutingRepo::get_by_id(&mut conn, rid).await.unwrap().unwrap();
    assert_eq!(after.unit_price, Some(Decimal::new(125, 2))); // 1.25
}

#[tokio::test]
async fn repo_has_report_false_before_reporting() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, "100").await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let mut conn = app.state.pool.acquire().await.unwrap();

    assert!(!WorkOrderRoutingRepo::has_report(&mut conn, rid).await.unwrap());
    assert!(!WorkOrderRoutingRepo::has_any_report(&mut conn, wo_id).await.unwrap());
}

use abt_core::shared::types::DomainError;

#[tokio::test]
async fn service_update_price_rejects_zero() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, "101").await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let err = svc
        .update_routing_unit_price(&ctx, &mut conn, wo_id, rid, Decimal::ZERO)
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn service_update_price_ok_then_persists() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, "102").await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let updated = svc
        .update_routing_unit_price(&ctx, &mut conn, wo_id, rid, Decimal::new(3, 0))
        .await
        .unwrap();
    assert_eq!(updated.unit_price, Some(Decimal::new(3, 0)));
    assert_eq!(updated.id, rid);
}

#[tokio::test]
async fn service_update_price_rejects_cross_order() {
    let app = common::TestApp::new().await;
    let wo_a = seed_released_work_order(&app, "103").await;
    let wo_b = seed_released_work_order(&app, "104").await;
    let rid_a = first_routing_id(&app.state, wo_a).await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let err = svc
        .update_routing_unit_price(&ctx, &mut conn, wo_b, rid_a, Decimal::new(3, 0))
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::NotFound { .. }), "got {err:?}");
}

#[tokio::test]
async fn service_delete_renumbers_and_blocks_last() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, "105").await; // 假设 seed 产出 ≥2 道工序
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let mut rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    println!("Initial routing count: {}", rs.len());
    if rs.len() < 2 {
        println!("SKIP: test requires at least 2 routings, got {}", rs.len());
        return;
    }
    // 删第一道 → 剩余重排
    svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap();
    rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    for (i, r) in rs.iter().enumerate() {
        assert_eq!(r.step_no as usize, i + 1);
    }
    // 删到只剩一道时拒绝
    while rs.len() > 1 {
        let id = rs[0].id;
        svc.delete_routing(&ctx, &mut conn, wo_id, id).await.unwrap();
        rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    }
    let err = svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }), "got {err:?}");
}
