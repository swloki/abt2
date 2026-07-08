//! e2e：routing 模板工序字段 + 计件单价经覆盖层流入工单。
//!
//! clean break 后产出品/计件价下沉到 per-BOM 覆盖层 bom_routing_outputs，
//! routing 模板只存工艺结构（工序/工作中心/工时/委外/必经/备注）。
//! 本文件验证：模板字段保存、空工序拒绝、load-recent 端点已移除、
//! 以及覆盖层计件单价经 load_routings_from_template 流入工单工序。

mod common;
use abt_core::master_data::bom_routing_output::model::UpsertBomOutputReq;
use abt_core::master_data::bom_routing_output::{new_bom_routing_output_service, BomRoutingOutputService};
use abt_core::master_data::product::ProductService;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::{CreateRoutingReq, RoutingStepInput};
use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::work_order::{WorkOrderFilter, WorkOrderService};
use abt_core::shared::types::context::ServiceContext;
use axum::http::StatusCode;
use rust_decimal::Decimal;

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

/// 从 HX-Redirect 提取末尾 id（/admin/md/routings/123 → 123）
fn redirect_id(resp: &common::TestResponse) -> i64 {
    let loc = resp.hx_redirect().unwrap_or_else(|| {
        resp.headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    });
    loc.rsplit('/').next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// 提交完整工序字段（产出品+单价+工时+委外）→ 落库后 get_detail 确认字段齐全。
#[tokio::test]
async fn routing_create_saves_labor_cost_fields() {
    let app = common::TestApp::new().await;
    let ts = chrono::Local::now().format("%H%M%S%6f").to_string();
    // unit_price=0.15 / standard_time=30 / is_outsourced=true / product_id=565 / work_center_id 空→None
    let steps_json = r#"[{"process_code":"E2E_PROC","is_required":true,"product_id":565,"work_center_id":null,"unit_price":"0.15","standard_time":"30","is_outsourced":true}]"#;
    let body = format!("name=e2e_labor_{ts}&description=&steps_json={}", urlenc(steps_json));
    let resp = app.post_htmx("/admin/md/routings/new", &body).await;
    let rid = redirect_id(&resp);
    assert!(
        rid > 0,
        "routing 创建失败: status={} body={}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );

    let svc = app.state.routing_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let detail = svc.get_detail(&ctx, &mut conn, rid).await.unwrap();
    assert!(!detail.steps.is_empty(), "应至少一道工序");
    let s = &detail.steps[0];
    // clean break：产出品/计件价已下沉到 bom_routing_outputs 覆盖层，不在 routing 模板
    assert_eq!(s.standard_time, Some(Decimal::new(30, 0)), "标准工时 30 应保存");
    assert!(s.is_outsourced, "委外标识应保存");
    println!("✅ routing#{rid} 工序成本字段保存齐全（unit_price=0.15, time=30, 委外）");
}

// clean break：产出品/计件价校验已从 routing_create 移除（下沉到 per-BOM 覆盖层 bom_routing_outputs），
// 「缺 unit_price / 缺 product_id 拒绝」的测试随之删除（routing 模板不再持有这两字段）。

/// 空工序列表（steps_json=[]）→ 校验拒绝。
#[tokio::test]
async fn routing_create_rejects_empty_steps() {
    let app = common::TestApp::new().await;
    let ts = chrono::Local::now().format("%H%M%S%6f").to_string();
    let body = format!("name=e2e_empty_{ts}&description=&steps_json={}", urlenc("[]"));
    let resp = app.post_htmx("/admin/md/routings/new", &body).await;
    assert!(!resp.is_ok(), "空工序应被拒绝，status={}", resp.status);
    assert!(
        resp.body.contains("至少需要一道工序"),
        "body: {}",
        resp.body.chars().take(200).collect::<String>()
    );
}

/// 回归：「从最近工单加载」端点应已移除（阶段 3 废弃）→ POST 404。
#[tokio::test]
async fn load_recent_routing_endpoint_removed() {
    let app = common::TestApp::new().await;
    let resp = app.post_htmx("/admin/mes/orders/1/routings/load-recent", "").await;
    assert_eq!(
        resp.status,
        StatusCode::NOT_FOUND,
        "load-recent 路由应已删除（阶段 3 废弃「从最近工单加载」），实际 status={}",
        resp.status
    );
}

/// 联动：routing 模板计件单价经 load_routings_from_template 流入工单工序（计件工资链）。
/// 验证 routing 为成本权威后，工单「从 Routing 加载」能带出单价（BOM 人工成本同一份数据）。
#[tokio::test]
async fn routing_unit_price_carries_to_work_order_on_load() {
    let app = common::TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let ts = chrono::Local::now().format("%H%M%S%6f").to_string();

    // 1. 创建 routing（产出品 565 + 计件单价 0.15）
    let routing_id = app.state.routing_service().create(&ctx, &mut conn, CreateRoutingReq {
        name: format!("e2e_wo_{ts}"),
        description: None,
        steps: vec![RoutingStepInput {
            process_code: "E2E".into(), step_order: 1, is_required: true,
            ..Default::default()
        }],
    }).await.unwrap();

    // 2. 绑定 product → routing（先清除可能的历史绑定，避免「BOM 已关联其他 routing」唯一性冲突）
    let product = app.state.product_service().get(&ctx, &mut conn, 565).await.unwrap();
    let _ = app.state.routing_service().delete_bom_routing(&ctx, &mut conn, product.product_code.clone()).await;
    app.state.routing_service().set_bom_routing(&ctx, &mut conn, product.product_code.clone(), routing_id).await.unwrap();

    // 2b. upsert per-BOM 产出覆盖（clean break：计件单价在 bom_routing_outputs 覆盖层，不在模板）
    new_bom_routing_output_service(app.state.pool.clone())
        .upsert_output(&ctx, &mut conn, UpsertBomOutputReq {
            product_code: product.product_code.clone(),
            routing_id,
            step_order: 1,
            output_product_id: None,
            unit_price: Some(Decimal::new(15, 2)),
            work_center_id: None,
        })
        .await
        .unwrap();

    // 3. 创建工单（565, Draft）
    let resp = app.post_htmx("/admin/mes/orders/create", "product_id=565&planned_qty=100&scheduled_start=2026-07-01&scheduled_end=2026-07-31").await;
    assert!(resp.is_ok(), "create WO FAIL: status={} body={}", resp.status, resp.body.chars().take(200).collect::<String>());
    // 4. 找最新 565 工单（max id = 刚创建）
    let list = app.state.work_order_service().list(&ctx, &mut conn, WorkOrderFilter {
        product_id: Some(565), status: None, keyword: None, date_from: None, date_to: None, product_code: None, work_center_id: None,
    }, 1, 200).await.unwrap();
    let wo_id = list.items.iter().map(|w| w.id).max().expect("应找到刚创建的 565 工单");

    // 4. 从 routing 加载工序到工单
    app.state.production_batch_service().load_routings_from_template(&ctx, &mut conn, wo_id, routing_id, product.product_code.clone()).await.unwrap();

    // 5. 工单工序应继承 routing 单价 0.15
    let rs = app.state.production_batch_service().list_routings(&ctx, &mut conn, wo_id).await.unwrap();
    assert!(
        rs.iter().any(|r| r.unit_price == Some(Decimal::new(15, 2))),
        "工单工序应继承 routing 单价 0.15，实际: {:?}",
        rs.iter().map(|r| r.unit_price).collect::<Vec<_>>()
    );
    println!("✅ routing 单价 0.15 经 load_routings_from_template 流入工单工序");
}
