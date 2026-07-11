//! 工单取消 → 需求回池集成测试
//!
//! 验证 cancel 工单时，关联 demand 经 DemandService::release_back_to_pool
//! 回池（status→Pending + 清 target_doc）并发布 DemandReleased 事件。
//! 对齐 Odoo：已开工可取消，入库闸门由 cancel 内部 receipt_count 校验。
//!
//! 测试隔离：共享远程 dev DB 不回滚，demand 用强唯一 source_id 隔离，结束手动 DELETE。

mod common;
use common::TestApp;

use abt_core::{
    mes::{
        enums::WorkOrderStatus,
        work_order::{CreateWorkOrderReq, WorkOrderService},
    },
    shared::{
        enums::{document_type::DocumentType, event::DomainEventType},
        types::ServiceContext,
    },
};
use chrono::NaiveDate;
use rust_decimal::Decimal;

const PRODUCT_ID: i64 = 565;

/// 建 Draft 工单，返回 wo_id。
async fn create_draft_wo(app: &TestApp) -> i64 {
    let svc = app.state.work_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.create(
        &ServiceContext::new(1),
        &mut conn,
        CreateWorkOrderReq {
            plan_item_id: None,
            product_id: PRODUCT_ID,
            bom_snapshot_id: None,
            routing_id: None,
            planned_qty: Decimal::from(10),
            scheduled_start: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            scheduled_end: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            work_center_id: None,
            sales_order_id: None,
            remark: None,
        },
    )
    .await
    .unwrap()
}

/// cancel 工单 → 关联 demand 应回池（Pending + 清 target_doc）并发布 DemandReleased 事件。
#[tokio::test]
async fn cancel_work_order_releases_demand_back_to_pool() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    // 强唯一 source_id（共享 dev DB 隔离）
    let unique_source: i64 = 9_000_071;
    let wo_id = create_draft_wo(&app).await;

    // 直接 SQL 插 demand fixture（demands 无 FK，source_id/source_line_id 用唯一值；
    // status=2 Confirmed，target_doc 指向工单，模拟"已创建计划"状态）
    let demand_id: i64 = sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO demands
             (demand_type, source_type, source_id, source_line_id, product_id,
              acquire_channel, required_qty, status, target_doc_type, target_doc_id, operator_id)
           VALUES (1, 2, $1, $1, $2, 1, 10, 2, $3, $4, 1)
           RETURNING id"#,
    )
    .bind(unique_source)
    .bind(PRODUCT_ID)
    .bind(DocumentType::WorkOrder as i16)
    .bind(wo_id)
    .fetch_one(&mut *conn)
    .await
    .unwrap();

    // 前置：工单为 Draft
    let wo = app
        .state
        .work_order_service()
        .find_by_id(&ctx, &mut conn, wo_id)
        .await
        .unwrap();
    assert_eq!(wo.status, WorkOrderStatus::Draft);

    // 取消工单 → 触发 release_back_to_pool（demand 回池 + 发 DemandReleased 事件）
    app.state
        .work_order_service()
        .cancel(&ctx, &mut conn, wo_id, wo.version)
        .await
        .unwrap();

    // 断言 1：demand 回 Pending + 清 target_doc
    let row: (i16, Option<i16>, Option<i64>) = sqlx::query_as(
        "SELECT status, target_doc_type, target_doc_id FROM demands WHERE id = $1",
    )
    .bind(demand_id)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(row.0, 1, "demand 应回 Pending(1) 重新进池");
    assert!(row.1.is_none(), "target_doc_type 应清空");
    assert!(row.2.is_none(), "target_doc_id 应清空");

    // 断言 2：DemandReleased 事件已落 domain_events
    let evt_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM domain_events WHERE event_type = $1 AND aggregate_id = $2",
    )
    .bind(DomainEventType::DemandReleased as i16)
    .bind(demand_id)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!(evt_count >= 1, "应发布 DemandReleased 事件");

    // cleanup（共享 dev DB 手动清理）
    let _ = sqlx::query(
        "DELETE FROM domain_events WHERE aggregate_id = $1 AND event_type = $2",
    )
    .bind(demand_id)
    .bind(DomainEventType::DemandReleased as i16)
    .execute(&mut *conn)
    .await;
    let _ = sqlx::query("DELETE FROM demands WHERE id = $1")
        .bind(demand_id)
        .execute(&mut *conn)
        .await;
    let _ = sqlx::query("DELETE FROM work_orders WHERE id = $1")
        .bind(wo_id)
        .execute(&mut *conn)
        .await;
}
