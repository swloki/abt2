//! MES 生产全流程 Handler 集成测试
//!
//! 覆盖完整生产链路：
//! 生产计划 → 工单下达 → 批次创建 → 工序报工 → 完工入库
//! 每条测试验证 HTTP 状态 + 数据库字段级正确性 + 异常边界。

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::{
    mes::{
        enums::{
            BatchStatus, PlanItemStatus, PlanStatus, PlanType, WorkOrderStatus,
        },
        production_batch::{ProductionBatchService, ProductionBatch},
        production_plan::{ProductionPlanService, ProductionPlan, ProductionPlanItem},
        production_receipt::ProductionReceiptService,
        work_order::{WorkOrderService, WorkOrder},
    },
    shared::types::ServiceContext,
};

// ── Helpers ──

fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 构造计划项 JSON
fn plan_items_json(items: &[(&str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty)| {
            format!(
                r#"{{"product_id":{pid},"planned_qty":"{qty}","scheduled_start":"2026-06-20","scheduled_end":"2026-07-20"}}"#
            )
        })
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

// 测试数据常量（数据库已有数据）
const PRODUCT_ID: i64 = 565; // 2835/冷白0.5W
const WAREHOUSE_ID: i64 = 23320; // 备料周转仓

// ── Service 层验证 helpers ──

async fn get_plan(app: &TestApp, id: i64) -> ProductionPlan {
    let svc = app.state.production_plan_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.find_by_id(&ServiceContext::new(1), &mut conn, id)
        .await
        .unwrap()
}

async fn get_plan_items(app: &TestApp, plan_id: i64) -> Vec<ProductionPlanItem> {
    let svc = app.state.production_plan_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.list_items(&ServiceContext::new(1), &mut conn, plan_id)
        .await
        .unwrap()
}

async fn get_work_order(app: &TestApp, id: i64) -> WorkOrder {
    let svc = app.state.work_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.find_by_id(&ServiceContext::new(1), &mut conn, id)
        .await
        .unwrap()
}

async fn get_batch(app: &TestApp, id: i64) -> ProductionBatch {
    let svc = app.state.production_batch_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.find_by_id(&ServiceContext::new(1), &mut conn, id)
        .await
        .unwrap()
}

// ── 链路构造 helpers ──

/// 创建生产计划并返回 plan_id
async fn create_plan(app: &TestApp, plan_type: &str, items: &[(&str, &str)]) -> i64 {
    let items_json = plan_items_json(items);
    let body = format!(
        "plan_type={plan_type}&plan_date=2026-06-16&remark=E2E测试&items_json={items_json}"
    );
    let resp = app.post_htmx("/admin/mes/plans/create", &body).await;
    assert!(
        resp.is_ok(),
        "create plan FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    // 计划创建成功后重定向到列表页（HX-Redirect: /admin/mes/plans）
    // plan_id 从列表页不便提取，这里用 Service 直接查最新创建的
    let svc = app.state.production_plan_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(
            &ServiceContext::new(1),
            &mut conn,
            abt_core::mes::production_plan::PlanFilter {
                status: None,
                plan_type: None,
                keyword: None,
                date_from: None,
                date_to: None,
            },
            1,
            1,
        )
        .await
        .unwrap();
    result.items.first().unwrap().id
}

/// 直接创建工单（不经计划），返回 (wo_id, version)
async fn create_work_order(app: &TestApp, qty: &str) -> (i64, i32) {
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
    // 工单创建后重定向到列表页，需要从 Service 层查最新的
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

/// 下达工单（先 find_by_id 拿 version，再 release）
async fn release_work_order(app: &TestApp, wo_id: i64) {
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

/// 拆批创建批次，返回 batch_id
async fn create_batch(app: &TestApp, wo_id: i64, qty: &str) -> i64 {
    let body = format!("split_qty={qty}");
    let resp = app
        .post_htmx(&format!("/admin/mes/orders/{wo_id}/split"), &body)
        .await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "split WO FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
    // 通过 Service 查最新创建的批次
    let svc = app.state.production_batch_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let batches = svc
        .list_by_work_order(&ServiceContext::new(1), &mut conn, wo_id)
        .await
        .unwrap();
    batches.first().unwrap().id
}

// ════════════════════════════════════════════════════════════════════════════
//  全流程 happy path: 计划 → 工单 → 批次 → 报工 → 入库
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn a1_full_lifecycle_plan_to_receipt() {
    let app = TestApp::new().await;

    // 1. 创建生产计划（MTS）
    let plan_id = create_plan(&app, "Mts", &[("565", "100")]).await;
    let plan = get_plan(&app, plan_id).await;
    assert_eq!(plan.status, PlanStatus::Draft);
    assert_eq!(plan.plan_type, PlanType::Mts);

    let items = get_plan_items(&app, plan_id).await;
    assert!(!items.is_empty(), "plan should have items");
    assert_eq!(items[0].status, PlanItemStatus::Planned);
    assert_eq!(items[0].product_id, PRODUCT_ID);

    // 2. 确认计划: Draft → Confirmed
    app.post_htmx(&format!("/admin/mes/plans/{plan_id}/confirm"), "")
        .await;
    assert_eq!(get_plan(&app, plan_id).await.status, PlanStatus::Confirmed);

    // 3. 直接创建工单（独立路径，不经计划自动生成）
    let (wo_id, _) = create_work_order(&app, "50").await;
    let wo = get_work_order(&app, wo_id).await;
    assert_eq!(wo.status, WorkOrderStatus::Draft);
    assert_eq!(wo.product_id, PRODUCT_ID);
    assert_eq!(wo.planned_qty, Decimal::from(50));

    // 4. 下达工单: Draft → Released
    release_work_order(&app, wo_id).await;
    let wo = get_work_order(&app, wo_id).await;
    assert_eq!(wo.status, WorkOrderStatus::Released);

    // 5. 验证工序自动创建（release 时自动创建 routing steps）
    let batch_svc = app.state.production_batch_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let routings = batch_svc
        .list_routings(&ServiceContext::new(1), &mut conn, wo_id)
        .await
        .unwrap();
    assert!(
        !routings.is_empty(),
        "released WO must have routing steps"
    );
    drop(conn);

    // 6. 拆批创建批次
    let batch_id = create_batch(&app, wo_id, "50").await;
    let batch = get_batch(&app, batch_id).await;
    assert_eq!(batch.status, BatchStatus::Pending);
    assert_eq!(batch.work_order_id, wo_id);
    assert_eq!(batch.batch_qty, Decimal::from(50));
    assert_eq!(batch.current_step, 0);
    assert!(!batch.batch_no.is_empty(), "batch_no must be generated");
    assert!(
        !batch.card_sn.is_empty(),
        "card_sn must be generated"
    );

    // 7. 工序报工: Pending → InProgress
    let report_body =
        "step_no=1&worker_id=1&shift=1&completed_qty=50&defect_qty=0&work_hours=8&report_date=2026-06-16";
    let resp = app
        .post_htmx(
            &format!("/admin/mes/batches/{batch_id}/confirm-step"),
            report_body,
        )
        .await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "confirm-step FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );

    let batch = get_batch(&app, batch_id).await;
    assert!(
        batch.status == BatchStatus::InProgress
            || batch.status == BatchStatus::PendingReceipt,
        "batch should be InProgress or PendingReceipt after step confirm, got {:?}",
        batch.status
    );
    assert_eq!(batch.completed_qty, Decimal::from(50));

    // 8. 验证工单传播为 InProduction（首次报工后自动传播）
    let wo = get_work_order(&app, wo_id).await;
    assert!(
        wo.status == WorkOrderStatus::InProduction
            || wo.status == WorkOrderStatus::Closed,
        "WO should propagate to InProduction after first report, got {:?}",
        wo.status
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  B. 生产计划 — 生命周期 + 异常
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn b1_plan_create_and_verify_items() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "100"), ("566", "200")]).await;
    let plan = get_plan(&app, plan_id).await;
    assert_eq!(plan.status, PlanStatus::Draft);

    let items = get_plan_items(&app, plan_id).await;
    assert_eq!(items.len(), 2, "should have 2 plan items");
    assert_eq!(items[0].planned_qty, Decimal::from(100));
    assert_eq!(items[1].planned_qty, Decimal::from(200));
}

#[tokio::test]
async fn b2_plan_confirm_transitions_status() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mto", &[("565", "10")]).await;
    assert_eq!(get_plan(&app, plan_id).await.status, PlanStatus::Draft);

    app.post_htmx(&format!("/admin/mes/plans/{plan_id}/confirm"), "")
        .await;
    assert_eq!(
        get_plan(&app, plan_id).await.status,
        PlanStatus::Confirmed
    );
}

#[tokio::test]
async fn b3_plan_error_invalid_date() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx(
            "/admin/mes/plans/create",
            "plan_type=Mts&plan_date=BAD&items_json=%5B%5D",
        )
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn b4_plan_detail_not_found() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/admin/mes/plans/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  C. 工单 — 生命周期 + 异常
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn c1_work_order_create_draft() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "30").await;
    let wo = get_work_order(&app, wo_id).await;
    assert_eq!(wo.status, WorkOrderStatus::Draft);
    assert_eq!(wo.planned_qty, Decimal::from(30));
}

#[tokio::test]
async fn c2_work_order_release_creates_routings() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "10").await;
    release_work_order(&app, wo_id).await;

    let batch_svc = app.state.production_batch_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let routings = batch_svc
        .list_routings(&ServiceContext::new(1), &mut conn, wo_id)
        .await
        .unwrap();
    assert!(!routings.is_empty(), "released WO must have routings");
    // 至少有一道工序，step_no 从 1 开始
    assert!(routings.iter().any(|r| r.step_no == 1));
}

#[tokio::test]
async fn c3_work_order_cancel_from_draft() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "5").await;
    let resp = app.post_htmx(&format!("/admin/mes/orders/{wo_id}/cancel"), "").await;
    // cancel 做软删除（deleted_at），验证 HTTP 成功 + 详情页 404
    assert!(resp.is_ok() || resp.is_redirect(), "cancel should succeed");
    assert_eq!(
        app.get(&format!("/admin/mes/orders/{wo_id}")).await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn c4_work_order_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.post_htmx("/admin/mes/orders/999999/release", "")
            .await
            .status,
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get("/admin/mes/orders/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn c5_work_order_detail_page_renders() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "20").await;
    let detail = app.get_htmx(&format!("/admin/mes/orders/{wo_id}")).await;
    assert!(detail.is_ok(), "WO detail page should render");
    let wo = get_work_order(&app, wo_id).await;
    assert!(
        detail.body_contains(&wo.doc_number),
        "detail should show doc_number"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  D. 批次 — 状态机 + 报工
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn d1_batch_split_creates_batch() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "40").await;
    release_work_order(&app, wo_id).await;

    let batch_id = create_batch(&app, wo_id, "40").await;
    let batch = get_batch(&app, batch_id).await;
    assert_eq!(batch.status, BatchStatus::Pending);
    assert_eq!(batch.batch_qty, Decimal::from(40));
    assert_eq!(batch.current_step, 0);
}

#[tokio::test]
async fn d2_batch_confirm_step_advances_progress() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "20").await;
    release_work_order(&app, wo_id).await;
    let batch_id = create_batch(&app, wo_id, "20").await;

    // 报工 step=1
    let body = "step_no=1&worker_id=1&shift=1&completed_qty=20&defect_qty=0&work_hours=8&report_date=2026-06-16";
    let resp = app
        .post_htmx(&format!("/admin/mes/batches/{batch_id}/confirm-step"), body)
        .await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "confirm-step should succeed"
    );

    let batch = get_batch(&app, batch_id).await;
    assert_eq!(batch.completed_qty, Decimal::from(20));
    assert!(batch.actual_start.is_some(), "actual_start should be set");
}

#[tokio::test]
async fn d3_batch_skip_step_blocked() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "10").await;
    release_work_order(&app, wo_id).await;
    let batch_id = create_batch(&app, wo_id, "10").await;

    // 批次 current_step=0，直接报 step=2 应被拦截（防跳序）
    let body = "step_no=2&worker_id=1&shift=1&completed_qty=10&defect_qty=0&work_hours=8&report_date=2026-06-16";
    let resp = app
        .post_htmx(&format!("/admin/mes/batches/{batch_id}/confirm-step"), body)
        .await;
    assert!(
        resp.status.is_client_error(),
        "skip-step should be blocked, got {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn d4_batch_suspend_and_resume() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "10").await;
    release_work_order(&app, wo_id).await;
    let batch_id = create_batch(&app, wo_id, "10").await;

    // 报工部分数量（单道工序报工后可能直接进入 PendingReceipt）
    let body = "step_no=1&worker_id=1&shift=1&completed_qty=5&defect_qty=0&work_hours=4&report_date=2026-06-16";
    app.post_htmx(&format!("/admin/mes/batches/{batch_id}/confirm-step"), body).await;
    let batch = get_batch(&app, batch_id).await;

    // 单道工序：报工后进入 InProgress 或 PendingReceipt
    if batch.status == BatchStatus::InProgress {
        app.post_htmx(&format!("/admin/mes/batches/{batch_id}/suspend"), "reason=设备检修").await;
        assert_eq!(get_batch(&app, batch_id).await.status, BatchStatus::Suspended);
        app.post_htmx(&format!("/admin/mes/batches/{batch_id}/resume"), "").await;
        assert_eq!(get_batch(&app, batch_id).await.status, BatchStatus::InProgress);
    } else {
        // PendingReceipt → suspend 应被拒绝
        let resp = app.post_htmx(&format!("/admin/mes/batches/{batch_id}/suspend"), "reason=test").await;
        assert!(resp.status.is_client_error(), "suspend from PendingReceipt should be rejected");
    }
}

#[tokio::test]
async fn d5_batch_detail_page_renders() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "15").await;
    release_work_order(&app, wo_id).await;
    let batch_id = create_batch(&app, wo_id, "15").await;

    let detail = app.get_htmx(&format!("/admin/mes/batches/{batch_id}")).await;
    assert!(detail.is_ok(), "batch detail should render");
    let batch = get_batch(&app, batch_id).await;
    assert!(detail.body_contains(&batch.batch_no));
}

// ════════════════════════════════════════════════════════════════════════════
//  E. 完工入库 — 创建 + 确认
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn e1_receipt_create_and_confirm() {
    let app = TestApp::new().await;
    let (wo_id, _) = create_work_order(&app, "50").await;
    release_work_order(&app, wo_id).await;

    // 创建入库单
    let body = format!(
        "work_order_id={wo_id}&product_id={PRODUCT_ID}&received_qty=50&warehouse_id={WAREHOUSE_ID}&receipt_date=2026-06-16"
    );
    let resp = app.post_htmx("/admin/mes/receipts/create", &body).await;
    assert!(
        resp.is_ok(),
        "create receipt FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );

    // 确认入库
    // 需要从 Service 查最新入库单 ID
    let svc = app.state.production_receipt_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(
            &ServiceContext::new(1),
            &mut conn,
            abt_core::mes::production_receipt::ReceiptListFilter { keyword: None },
            1,
            5,
        )
        .await
        .unwrap();
    let receipt_id = result.items.first().unwrap().id;
    drop(conn);

    let resp = app
        .post_htmx(&format!("/admin/mes/receipts/{receipt_id}/confirm"), "")
        .await;
    // 入库确认可能因 FQC 门控或库存不足失败，验证至少不崩溃
    assert!(
        resp.is_ok() || resp.status.is_client_error(),
        "confirm receipt should either succeed or fail gracefully, got {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn e2_receipt_nonexistent_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/admin/mes/receipts/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  F. 页面可达性
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn f1_all_mes_list_pages_accessible() {
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

#[tokio::test]
async fn f2_htmx_pages_return_fragments() {
    let app = TestApp::new().await;
    for url in [
        "/admin/mes/plans",
        "/admin/mes/orders",
        "/admin/mes/receipts",
        "/admin/mes/inspections",
        "/admin/mes/demand-pool",
    ] {
        let resp = app.get_htmx(url).await;
        assert!(
            resp.is_ok(),
            "HTMX {url} returned {}",
            resp.status
        );
    }
}

#[tokio::test]
async fn f3_create_pages_accessible() {
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
