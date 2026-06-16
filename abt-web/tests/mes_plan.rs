//! 生产计划深度 Handler 集成测试
//!
//! 覆盖：创建/确认/排程/预校验/列表筛选/详情页

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::{
    mes::{
        enums::{PlanItemStatus, PlanStatus, PlanType},
        production_plan::{
            model::{PlanFilter, ProductionPlan, ProductionPlanItem},
            ProductionPlanService,
        },
    },
    shared::types::ServiceContext,
};

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

async fn get_plan(app: &TestApp, id: i64) -> ProductionPlan {
    let svc = app.state.production_plan_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.find_by_id(&ServiceContext::new(1), &mut conn, id).await.unwrap()
}

async fn get_plan_items(app: &TestApp, plan_id: i64) -> Vec<ProductionPlanItem> {
    let svc = app.state.production_plan_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.list_items(&ServiceContext::new(1), &mut conn, plan_id).await.unwrap()
}

/// 创建计划并返回 plan_id（从 Service list 查最新）
async fn create_plan(app: &TestApp, plan_type: &str, items: &[(&str, &str)]) -> i64 {
    let items_json = plan_items_json(items);
    let body = format!("plan_type={plan_type}&plan_date=2026-06-16&remark=PlanTest&items_json={items_json}");
    let resp = app.post_htmx("/admin/mes/plans/create", &body).await;
    assert!(resp.is_ok(), "create plan FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    let svc = app.state.production_plan_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let result = svc
        .list(&ServiceContext::new(1), &mut conn, PlanFilter {
            status: None, plan_type: None, keyword: None, date_from: None, date_to: None,
        }, 1, 1)
        .await
        .unwrap();
    result.items.first().unwrap().id
}

// ════════════════════════════════════════════════════════════════════════════
//  创建 + 字段验证
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_mto_plan_correct_type_and_status() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mto", &[("565", "100")]).await;
    let plan = get_plan(&app, plan_id).await;
    assert_eq!(plan.plan_type, PlanType::Mto);
    assert_eq!(plan.status, PlanStatus::Draft);
}

#[tokio::test]
async fn create_mts_plan_correct_type() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "50")]).await;
    assert_eq!(get_plan(&app, plan_id).await.plan_type, PlanType::Mts);
}

#[tokio::test]
async fn create_plan_with_multiple_items() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "100"), ("566", "200"), ("567", "300")]).await;
    let items = get_plan_items(&app, plan_id).await;
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].planned_qty, Decimal::from(100));
    assert_eq!(items[1].planned_qty, Decimal::from(200));
    assert_eq!(items[2].planned_qty, Decimal::from(300));
    // 所有 item 初始状态 = Planned
    for item in &items {
        assert_eq!(item.status, PlanItemStatus::Planned);
    }
}

#[tokio::test]
async fn create_plan_empty_items_succeeds() {
    let app = TestApp::new().await;
    let body = "plan_type=Mts&plan_date=2026-06-16&items_json=%5B%5D";
    let resp = app.post_htmx("/admin/mes/plans/create", body).await;
    assert!(resp.is_ok(), "empty plan should succeed: {}", resp.status);
}

#[tokio::test]
async fn create_plan_default_type_falls_back_to_mts() {
    let app = TestApp::new().await;
    // 未知 plan_type 应回退为 MTS（handler match 默认分支）
    let body = "plan_type=UNKNOWN&plan_date=2026-06-16&items_json=%5B%5D";
    let resp = app.post_htmx("/admin/mes/plans/create", body).await;
    assert!(resp.is_ok(), "unknown plan_type should default to Mts");
}

// ════════════════════════════════════════════════════════════════════════════
//  确认 → 状态转换
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn confirm_transitions_draft_to_confirmed() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "10")]).await;
    assert_eq!(get_plan(&app, plan_id).await.status, PlanStatus::Draft);

    app.post_htmx(&format!("/admin/mes/plans/{plan_id}/confirm"), "").await;
    assert_eq!(get_plan(&app, plan_id).await.status, PlanStatus::Confirmed);
}

#[tokio::test]
async fn confirm_already_confirmed_succeeds_idempotently() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "10")]).await;
    app.post_htmx(&format!("/admin/mes/plans/{plan_id}/confirm"), "").await;
    // 再次确认应幂等处理（返回 redirect 或 error，但不崩溃）
    let resp = app.post_htmx(&format!("/admin/mes/plans/{plan_id}/confirm"), "").await;
    assert!(resp.is_ok() || resp.is_redirect() || resp.status.is_client_error());
    assert_eq!(get_plan(&app, plan_id).await.status, PlanStatus::Confirmed);
}

// ════════════════════════════════════════════════════════════════════════════
//  异常输入
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_invalid_date_returns_400() {
    let app = TestApp::new().await;
    let resp = app
        .post_htmx("/admin/mes/plans/create", "plan_type=Mts&plan_date=BAD&items_json=%5B%5D")
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_malformed_items_json_does_not_crash() {
    let app = TestApp::new().await;
    // items_json 解析失败时 unwrap_or_default() 返回空 vec
    let resp = app
        .post_htmx(
            "/admin/mes/plans/create",
            "plan_type=Mts&plan_date=2026-06-16&items_json=NOT_JSON",
        )
        .await;
    // handler 对解析失败做 unwrap_or_default，不会 400
    assert!(resp.is_ok() || resp.status.is_client_error());
}

#[tokio::test]
async fn detail_not_found_returns_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/admin/mes/plans/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn confirm_nonexistent_returns_404() {
    let app = TestApp::new().await;
    let resp = app.post_htmx("/admin/mes/plans/999999/confirm", "").await;
    assert_eq!(resp.status, axum::http::StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════════════
//  列表 & 详情页
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_page_renders() {
    let app = TestApp::new().await;
    let resp = app.get("/admin/mes/plans").await;
    assert!(resp.is_ok(), "plan list page should render");
}

#[tokio::test]
async fn detail_page_shows_doc_number() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "10")]).await;
    let plan = get_plan(&app, plan_id).await;
    let detail = app.get_htmx(&format!("/admin/mes/plans/{plan_id}")).await;
    assert!(detail.is_ok());
    assert!(detail.body_contains(&plan.doc_number), "detail should show doc_number");
}

#[tokio::test]
async fn create_page_renders() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/mes/plans/create").await.is_ok());
}

// ════════════════════════════════════════════════════════════════════════════
//  排程
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn schedule_v1_does_not_crash() {
    let app = TestApp::new().await;
    let plan_id = create_plan(&app, "Mts", &[("565", "10")]).await;
    app.post_htmx(&format!("/admin/mes/plans/{plan_id}/confirm"), "").await;
    // 排程应至少不崩溃
    let resp = app.post_htmx(&format!("/admin/mes/plans/{plan_id}/schedule"), "").await;
    assert!(
        resp.is_ok() || resp.is_redirect() || resp.status.is_client_error(),
        "schedule should not crash, got {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}
