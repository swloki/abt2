//! Issue#67 委外单联动集成测试：outsourcing_summary + suggest-materials
mod common;

use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::om::outsourcing_order::OutsourcingOrderService;
use abt_core::shared::types::context::ServiceContext;

const MULTI_STEP_PRODUCT_ID: i64 = 4544; // 与 mes_routing_price 一致（19 道工序）

async fn create_work_order(app: &common::TestApp, product_id: i64, qty: &str) -> i64 {
 let body = format!(
 "product_id={product_id}&planned_qty={qty}&scheduled_start=2026-06-20&scheduled_end=2026-07-20"
 );
 let resp = app.post_htmx("/admin/mes/orders/create", &body).await;
 assert!(resp.is_ok(), "create WO FAIL: {}", resp.status);
 let svc = app.state.work_order_service();
 let mut conn = app.state.pool.acquire().await.unwrap();
 let result = svc
 .list(
 &ServiceContext::new(1), &mut conn,
 abt_core::mes::work_order::WorkOrderFilter {
 status: None, product_id: None, keyword: None, date_from: None, date_to: None,
 },
 1, 1,
 )
 .await
 .unwrap();
 result.items.first().unwrap().id
}

async fn seed_released(app: &common::TestApp, product_id: i64, qty: &str) -> i64 {
 let wo_id = create_work_order(app, product_id, qty).await;
 let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/release"), "").await;
 assert!(resp.is_ok() || resp.is_redirect(), "release FAIL: {}", resp.status);
 wo_id
}

#[tokio::test]
async fn outsourcing_summary_returns_wo_fields_and_routings() {
 let app = common::TestApp::new().await;
 let wo_id = seed_released(&app, MULTI_STEP_PRODUCT_ID, "100").await;
 let svc = app.state.outsourcing_order_service();
 let ctx = ServiceContext::new(1);
 let mut conn = app.state.pool.acquire().await.unwrap();
 let s = svc.outsourcing_summary(&ctx, &mut conn, wo_id).await.unwrap();
 assert!(s.planned_qty > rust_decimal::Decimal::ZERO);
 assert!(!s.routings.is_empty(), "应返回工序列表供下拉渲染");
}

#[tokio::test]
async fn suggest_materials_without_product_id_returns_hint() {
 let app = common::TestApp::new().await;
 let wo_id = seed_released(&app, MULTI_STEP_PRODUCT_ID, "100").await;
 // 取一道工序（默认无 product_id）
 let batch_svc = app.state.production_batch_service();
 let ctx = ServiceContext::new(1);
 let mut conn = app.state.pool.acquire().await.unwrap();
 let rs = batch_svc.list_routings(&ctx, &mut conn, wo_id).await.unwrap();
 let rid = rs[0].id;
 // HTTP: 该工序无产出品 → 业务错误提示
 let resp = app
 .get(&format!(
 "/admin/om/outsourcing/suggest-materials?work_order_id={wo_id}&routing_id={rid}&planned_qty=10"
 ))
 .await;
 assert!(
 resp.body_contains("未关联产出品"),
 "无产出品应提示维护，got status {} body {}",
 resp.status,
 resp.body.chars().take(200).collect::<String>()
 );
}
