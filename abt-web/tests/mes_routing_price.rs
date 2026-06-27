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
use abt_core::mes::production_batch::{
    CreateBatchReq, ProductionBatchService, StepConfirmationReq,
};
use abt_core::mes::enums::ShiftType;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::repo::WorkReportRepo;
use abt_core::shared::types::context::ServiceContext;
use chrono::NaiveDate;
use rust_decimal::Decimal;

const PRODUCT_ID: i64 = 565; // 2835/冷白0.5W（与 mes_flow_e2e.rs 一致；release 时生成默认 1 道工序）
const MULTI_STEP_PRODUCT_ID: i64 = 4544; // 老款G系列联保灌胶电源，工艺路径 19 道工序（用于删除/重排测试）

/// 创建工单（复用 mes_flow_e2e.rs 逻辑）
async fn create_work_order(app: &common::TestApp, product_id: i64, qty: &str) -> (i64, i32) {
    let body = format!(
        "product_id={product_id}&planned_qty={qty}&scheduled_start=2026-06-20&scheduled_end=2026-07-20"
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
                date_to: None, product_code: None, work_center_id: None,
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
async fn seed_released_work_order(app: &common::TestApp, product_id: i64, qty: &str) -> i64 {
    let (wo_id, _) = create_work_order(app, product_id, qty).await;
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
    let wo_id = seed_released_work_order(&app, PRODUCT_ID, "100").await;
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
    let wo_id = seed_released_work_order(&app, PRODUCT_ID, "100").await;
    let rid = first_routing_id(&app.state, wo_id).await;
    let mut conn = app.state.pool.acquire().await.unwrap();

    assert!(!WorkOrderRoutingRepo::has_report(&mut conn, rid).await.unwrap());
    assert!(!WorkOrderRoutingRepo::has_any_report(&mut conn, wo_id).await.unwrap());
}

use abt_core::shared::types::DomainError;

#[tokio::test]
async fn service_update_routing_saves_both_and_guards() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "800").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let rid = rs[0].id;

    // 正常：一次保存 product_id + unit_price
    let updated = svc
        .update_routing(&ctx, &mut conn, wo_id, rid, Some(565), Decimal::new(7, 0), None, None, false)
        .await
        .unwrap();
    assert_eq!(updated.product_id, Some(565));
    assert_eq!(updated.unit_price, Some(Decimal::new(7, 0)));

    // 守卫：单价 ≤ 0
    let err = svc.update_routing(&ctx, &mut conn, wo_id, rid, None, Decimal::ZERO, None, None, false).await.unwrap_err();
    assert!(matches!(err, DomainError::Validation { .. }), "got {err:?}");

    // 守卫：跨工单
    let wo_b = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "801").await;
    let err = svc.update_routing(&ctx, &mut conn, wo_b, rid, None, Decimal::new(3, 0), None, None, false).await.unwrap_err();
    assert!(matches!(err, DomainError::NotFound { .. }), "got {err:?}");
}

#[tokio::test]
async fn service_delete_renumbers_and_blocks_last() {
    let app = common::TestApp::new().await;
    // 用多工序产品（19 道），才能真正覆盖 删除→重排→保留≥1 守卫
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "105").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let mut rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    assert!(rs.len() >= 2, "多工序产品应 ≥2 道，实际 {}", rs.len());

    // 删第一道 → 剩余 step_no 应重排为连续 0..N-1（与模板 step_order 一致的 0-based）
    svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap();
    rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    for (i, r) in rs.iter().enumerate() {
        assert_eq!(r.step_no as usize, i, "重排后 step_no 应 0-based 连续");
    }

    // 逐道删，直到只剩一道 → 再删应被守卫拒绝
    while rs.len() > 1 {
        let id = rs[0].id;
        svc.delete_routing(&ctx, &mut conn, wo_id, id).await.unwrap();
        rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    }
    let err = svc.delete_routing(&ctx, &mut conn, wo_id, rs[0].id).await.unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }), "删最后一道应拒绝，got {err:?}");
}

#[tokio::test]
async fn detail_page_shows_editable_price_and_delete_when_unreported() {
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "200").await;
    let resp = app.get(&format!("/admin/mes/orders/{wo_id}")).await;
    assert!(
        resp.is_ok(),
        "detail GET FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    assert!(resp.body_contains("/edit"), "未报工工单应渲染编辑按钮");
    assert!(resp.body_contains("/delete"), "未报工工单应渲染删除端点");
    assert!(resp.body_contains("#routing-edit-drawer"), "应渲染编辑抽屉壳");
}

#[tokio::test]
async fn wage_is_frozen_at_report_time() {
    let app = common::TestApp::new().await;
    let batch_svc = app.state.production_batch_service();
    let wo_svc = app.state.work_order_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    // 建多工序工单 + 下达 + 建批次
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "300").await;
    let wo = wo_svc.find_by_id(&ctx, &mut conn, wo_id).await.unwrap();
    let batch_id = batch_svc
        .create(
            &ctx, &mut conn,
            CreateBatchReq { work_order_id: wo_id, product_id: wo.product_id, batch_qty: Decimal::new(100, 0), team_id: None },
        )
        .await
        .unwrap();

    // 设第 1 道工序单价 = 5
    let rs = batch_svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let step1 = rs.iter().find(|r| r.step_no == 1).unwrap();
    batch_svc
        .update_routing(&ctx, &mut conn, wo_id, step1.id, None, Decimal::new(5, 0), None, None, false)
        .await
        .unwrap();

    // 报工：完成 10 件 → 工资应冻结为 50
    let result = batch_svc
        .confirm_routing_step(
            &ctx, &mut conn, batch_id, 1,
            StepConfirmationReq {
                step_no: 1,
                worker_id: 1,
                shift: ShiftType::Day,
                completed_qty: Decimal::new(10, 0),
                defect_qty: Decimal::ZERO,
                defect_reason: None,
                work_hours: Decimal::new(1, 0),
                report_date: NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
                remark: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.wage_amount, Decimal::new(50, 0), "报工应按当时单价冻结工资");

    // 落库验证：work_reports.wage_amount 冻结为 50
    let reports = WorkReportRepo::list_by_batch(&mut conn, batch_id).await.unwrap();
    let wr = reports.iter().find(|r| r.routing_id == step1.id).unwrap();
    assert_eq!(wr.wage_amount, Decimal::new(50, 0), "wage_amount 应已冻结落库");

    // 报工后改该工序单价 → 应被守卫拒绝（不会污染历史工资）
    let err = batch_svc
        .update_routing(&ctx, &mut conn, wo_id, step1.id, None, Decimal::new(9, 0), None, None, false)
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }), "报工后改价应拒绝");
}

#[tokio::test]
async fn delete_blocked_after_any_report() {
    let app = common::TestApp::new().await;
    let batch_svc = app.state.production_batch_service();
    let wo_svc = app.state.work_order_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "400").await;
    let wo = wo_svc.find_by_id(&ctx, &mut conn, wo_id).await.unwrap();
    let batch_id = batch_svc
        .create(
            &ctx, &mut conn,
            CreateBatchReq { work_order_id: wo_id, product_id: wo.product_id, batch_qty: Decimal::new(100, 0), team_id: None },
        )
        .await
        .unwrap();
    // 报工第 1 道
    batch_svc
        .confirm_routing_step(
            &ctx, &mut conn, batch_id, 1,
            StepConfirmationReq {
                step_no: 1, worker_id: 1, shift: ShiftType::Day,
                completed_qty: Decimal::new(5, 0), defect_qty: Decimal::ZERO,
                defect_reason: None, work_hours: Decimal::new(1, 0),
                report_date: NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(), remark: None,
            },
        )
        .await
        .unwrap();

    // 工单已有报工 → 删除任意工序应被全局守卫拒绝
    let rs = batch_svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    let target = rs.iter().find(|r| r.step_no == 2).unwrap();
    let err = batch_svc.delete_routing(&ctx, &mut conn, wo_id, target.id).await.unwrap_err();
    assert!(matches!(err, DomainError::BusinessRule { .. }), "有报工后删工序应拒绝，got {err:?}");
}


#[tokio::test]
async fn load_routings_from_template_replaces_steps() {
    use abt_core::master_data::routing::RoutingService;
    let app = common::TestApp::new().await;
    let wo_id = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "950").await;
    let svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    // 工单 release 时应该有 routing_id
    let wo = app.state.work_order_service().find_by_id(&ctx, &mut conn, wo_id).await.unwrap();
    let routing_id = wo.routing_id.expect("工单应有关联的工艺路径");
    // 加载模板步骤
    let n = svc.load_routings_from_template(&ctx, &mut conn, wo_id, routing_id).await.unwrap();
    assert!(n > 0, "应至少插入 1 行，实际 {n}");
    // 验证加载后的工序行
    let rs = svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    assert_eq!(rs.len(), n, "加载后的行数应与插入数一致");
    // 验证模板步骤结构与工单工序对应
    let detail = app.state.routing_service().get_detail(&ctx, &mut conn, routing_id).await.unwrap();
    assert_eq!(rs.len(), detail.steps.len(), "工单工序行数应与模板步骤数一致");
    for (r, s) in rs.iter().zip(detail.steps.iter()) {
        assert_eq!(r.step_no, s.step_order, "step_no 应对齐");
        assert!(!r.process_name.is_empty(), "工序名不应为空");
        assert_eq!(r.is_outsourced, s.is_outsourced);
        assert_eq!(r.is_inspection_point, s.is_inspection_point);
    }
}

#[tokio::test]
async fn load_routings_from_recent_copies_sibling() {
    let app = common::TestApp::new().await;
    let batch_svc = app.state.production_batch_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let wo_a = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "910").await;
    let wo_b = seed_released_work_order(&app, MULTI_STEP_PRODUCT_ID, "911").await;
    let rs_a = batch_svc.list_routings(&ctx, &mut conn, wo_a).await.unwrap();
    batch_svc.update_routing(&ctx, &mut conn, wo_a, rs_a[0].id, Some(565), Decimal::new(5,0), None, None, false).await.unwrap();
    let n = batch_svc.load_routings_from_recent(&ctx, &mut conn, wo_b).await.unwrap();
    assert!(n >= 1, "应至少复制 1 行，实际 {n}");
    let rs_b = batch_svc.list_routings(&ctx, &mut conn, wo_b).await.unwrap();
    assert_eq!(rs_b[0].product_id, Some(565), "B 应从 A 复制产出品");
}
