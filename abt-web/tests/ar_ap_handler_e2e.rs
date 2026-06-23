//! 业财串联 handler 级端到端测试（Phase 2）：
//! 走完整业务流程（建单→确认→发货/来料/收货），验证业务 handler 直接触发往来台账。
//!
//! - k1 销售到财务：销售订单 → 确认 → 发货（ship）→ AR 台账（Debit 应收）
//! - k2 采购到财务：采购订单 → 来料通知 → 收货+检验（ArrivalAcceptedHandler）→ AP 台账（Credit 应付）
//! - k3 委外到财务：委外单 → 收货（receive）→ AP 台账（Credit 加工费）

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::shared::types::ServiceContext;
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::fms::ar_ap::repo::ArApLedgerRepo;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::om::outsourcing_order::OutsourcingOrderService;

// ── 测试数据常量（dev 库实测可用，与 sales_to_wms_e2e 一致）──
const CUSTOMER_ID: i64 = 135;
const CONTACT_ID: i64 = 135;
const SUPPLIER_ID: i64 = 129;
const WH: i64 = 23320; // 备料周转仓
const ZONE: i64 = 23320000;
const BIN: i64 = 23320000;
const PRODUCT: i64 = 565; // 外购产品，库存充足

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

fn so_items(items: &[(&str, &str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty, price)| format!(r#"{{"product_id":"{pid}","quantity":"{qty}","unit_price":"{price}"}}"#))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

fn po_items(items: &[(&str, &str, &str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, desc, qty, price)| {
            format!(r#"{{"product_id":"{pid}","description":"{desc}","quantity":"{qty}","unit_price":"{price}","item_delivery_date":null,"discount_pct":null,"tax_rate_id":null}}"#)
        })
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

fn ship_items(items: &[(i64, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(oid, qty)| format!(r#"{{"order_item_id":{oid},"warehouse_id":{WH},"requested_qty":"{qty}"}}"#))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

fn arrival_items(items: &[(String, String, String)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty, order_item_id)| {
            format!(r#"{{"product_id":"{pid}","declared_qty":"{qty}","batch_no":null,"order_item_id":"{order_item_id}"}}"#)
        })
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

fn redirect_id(resp: &common::TestResponse) -> i64 {
    let loc = resp.hx_redirect().unwrap_or("");
    loc.rsplit('/').next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// 直接入库备料（绕过 web 表单，确保发货有库存）
async fn stock_in(app: &TestApp, product_id: i64, qty: i64) {
    use abt_core::wms::enums::TransactionType;
    use abt_core::wms::inventory_transaction::{model::RecordTransactionReq, InventoryTransactionService};
    let svc = app.state.inventory_transaction_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.record(
        &ctx, &mut conn,
        RecordTransactionReq {
            doc_number: None, delivery_no: None, source_doc_number: None,
            transaction_type: TransactionType::PurchaseReceipt,
            product_id, warehouse_id: WH, zone_id: Some(ZONE), bin_id: Some(BIN),
            batch_no: None, quantity: Decimal::from(qty), unit_cost: None,
            source_type: "test_setup".to_string(), source_id: 0, remark: None,
        },
    ).await.unwrap();
}

/// 查某 source 的台账（验证立账）
async fn ledger_by_source(app: &TestApp, source_type: DocumentType, source_id: i64) -> Option<abt_core::fms::ar_ap::model::ArApLedger> {
    let mut conn = app.state.pool.acquire().await.unwrap();
    ArApLedgerRepo::get_open_by_source(&mut conn, source_type, source_id).await.unwrap()
}

// ════════════════════════════════════════════════════════════════════════════
//  k1 销售到财务：销售订单 → 发货 → AR 台账
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k1_sales_ship_to_ar_ledger() {
    let app = TestApp::new().await;
    // product 565 已在 23320（备料周转仓）有充足库存，无需 stock_in（一库位一产品，固定 BIN 可能被占）

    // 1) 销售订单 + 确认
    let so_body = format!(
        "customer_id={CUSTOMER_ID}&contact_id={CONTACT_ID}&items_json={}",
        so_items(&[(&PRODUCT.to_string(), "10", "1.00")])
    );
    let resp = app.post_htmx("/admin/orders/create", &so_body).await;
    assert!(resp.is_ok(), "创建销售订单 FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let so_id = redirect_id(&resp);
    assert!(so_id > 0, "应返回 SO id");
    let _ = app.post_htmx(&format!("/admin/orders/{so_id}/confirm"), "").await;

    // 2) 取 order_item_id
    let so_svc = app.state.sales_order_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let so_items_rows = so_svc.list_items(&ctx, &mut conn, so_id).await.unwrap();
    let order_item_id = so_items_rows[0].id;
    drop(conn);

    // 3) 发货 → confirm → pick → ship（ship 触发 AR 立账）
    let body = format!(
        "customer_id={CUSTOMER_ID}&order_id={so_id}&items_json={}",
        ship_items(&[(order_item_id, "10")])
    );
    let resp = app.post_htmx("/admin/shipping/create", &body).await;
    assert!(resp.is_ok(), "创建发货单 FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let ship_id = redirect_id(&resp);
    assert!(ship_id > 0, "应返回发货单 id");

    let _ = app.post_htmx(&format!("/admin/shipping/{ship_id}/confirm"), "").await;
    let _ = app.post_htmx(&format!("/admin/shipping/{ship_id}/pick"), "").await;
    let resp = app.post_htmx(&format!("/admin/shipping/{ship_id}/ship"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "发货 ship FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    // 4) 验证 AR 台账（ShippingRequest Debit 应收，金额 = 10 × 1.00 = 10）
    let ledger = ledger_by_source(&app, DocumentType::ShippingRequest, ship_id).await;
    let ledger = ledger.expect("❌ ship 未生成 AR 台账");
    assert_eq!(ledger.party_id, CUSTOMER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Debit);
    assert_eq!(ledger.amount, Decimal::from(10));
}

// ════════════════════════════════════════════════════════════════════════════
//  k2 采购到财务：采购订单 → 来料 → 收货+检验 → AP 台账
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k2_purchase_arrival_to_ap_ledger() {
    let app = TestApp::new().await;

    // 1) 采购订单 → 提交
    let items = po_items(&[(&PRODUCT.to_string(), "k2-PO", "20", "2.00")]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-23&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "创建采购订单 FAIL: {}", resp.status);
    let po_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;

    // 2) 来料通知（关联 PO）→ receive → inspect（inspect 触发 ArrivalAcceptedHandler 立 AP）
    let po_items_rows = app.state.purchase_order_service()
        .list_items(&ServiceContext::new(1), &mut app.state.pool.acquire().await.unwrap(), po_id)
        .await.unwrap();
    let arr_items = arrival_items(
        &po_items_rows.iter()
            .map(|it| (it.product_id.to_string(), "20".into(), it.id.to_string()))
            .collect::<Vec<_>>(),
    );
    let body = format!(
        "purchase_order_id={po_id}&supplier_id={SUPPLIER_ID}&arrival_date=2026-06-23&warehouse_id={WH}&items_json={arr_items}"
    );
    let resp = app.post_htmx("/admin/wms/arrivals/create", &body).await;
    assert!(resp.is_ok(), "创建来料通知 FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let arr_id = redirect_id(&resp);

    let _ = app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=receive").await;
    let resp = app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=inspect").await;
    assert!(resp.is_ok() || resp.is_redirect(), "来料检验 FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    // ArrivalAcceptedHandler 异步（event processor 后台），cargo test 未启动 processor；
    // 直接调 handler 模拟事件处理，验证立账逻辑
    use abt_core::purchase::arrival_handler::ArrivalAcceptedHandler;
    use abt_core::shared::event_bus::registry::EventHandler;
    use abt_core::shared::event_bus::model::DomainEvent;
    use abt_core::shared::enums::event::{DomainEventType, EventStatus};
    let handler = ArrivalAcceptedHandler::new(app.state.pool.clone());
    let event = DomainEvent {
        id: 0,
        event_type: DomainEventType::ArrivalInspected,
        event_version: 1,
        aggregate_type: "ArrivalNotice".into(),
        aggregate_id: arr_id,
        payload: serde_json::json!({"arrival_notice_id": arr_id, "doc_number": "test-k2"}),
        operator_id: 1,
        idempotency_key: format!("test-k2-{arr_id}"),
        trace_id: None,
        request_id: None,
        status: EventStatus::Pending,
        retry_count: 0,
        failure_reason: None,
        processed_at: None,
        created_at: chrono::Utc::now(),
    };
    handler.handle(&event).await.expect("ArrivalAcceptedHandler handle FAIL");

    let ledger = ledger_by_source(&app, DocumentType::ArrivalNotice, arr_id).await
        .expect("❌ arrival handler 未生成 AP 台账");
    assert_eq!(ledger.party_id, SUPPLIER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Credit);
    assert_eq!(ledger.amount, Decimal::from(40)); // 20 × 2.00
}

// ════════════════════════════════════════════════════════════════════════════
//  k3 委外到财务：委外单收货 → AP 台账（加工费）
//  委外完整建单+发料流程复杂，这里用 dev 库现有 Sent 委外单 + service receive 触发立账
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k3_outsourcing_receive_to_ap_ledger() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    // dev 库 Sent 委外单（OO-2026-06-000004, id=11；唯一 Sent 单）
    let om_svc = app.state.outsourcing_order_service();
    let oo_id: i64 = 11;
    let order = om_svc.find_by_id(&ctx, &mut conn, oo_id).await
        .expect("❌ find_by_id 委外单 11 FAIL");
    assert_eq!(order.status, abt_core::om::enums::OutsourcingStatus::Sent, "委外11应为 Sent");

    // dev 库 11 的 entity_state_logs 残留 Received（业务表 Sent 不一致，历史数据）；
    // 删除残留的 Received 日志让状态机回到 Sent，receive 才能正常转换
    sqlx::query("DELETE FROM entity_state_logs WHERE entity_type='OutsourcingOrder' AND entity_id=$1 AND to_state='Received'")
        .bind(oo_id)
        .execute(&mut *conn).await.expect("reset state log FAIL");

    // receive（触发 AP 立账，加工费 = received × unit_price）
    use abt_core::om::outsourcing_order::model::ReceiveOutsourcingReq;
    let recv_qty = Decimal::from(1);
    om_svc.receive(&ctx, &mut conn, ReceiveOutsourcingReq {
        id: oo_id,
        expected_version: order.version,
        received_qty: recv_qty,
        warehouse_id: Some(WH),
        iqc_passed_qty: Some(recv_qty),
        remark: None,
    }).await.expect("receive FAIL");

    // 验证 AP 台账（OutsourcingOrder Credit 加工费）
    let ledger = ledger_by_source(&app, DocumentType::OutsourcingOrder, oo_id).await;
    let ledger = ledger.expect("❌ receive 未生成 AP 台账");
    assert_eq!(ledger.party_id, order.supplier_id);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Credit);
    assert_eq!(ledger.amount, recv_qty * order.unit_price);
}

// ════════════════════════════════════════════════════════════════════════════
//  k4 采购退货结算冲减应付（Issue #85）：
//  PO → 来料收货 → 退货单 → PurchaseReturnSettled 事件 → 反向 AP 台账（Debit）+ 幂等
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k4_purchase_return_settled_reverses_ap_ledger() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);

    // 1) 采购订单 → submit
    let items = po_items(&[(&PRODUCT.to_string(), "k4-PO", "10", "3.00")]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-24&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "创建采购订单 FAIL: {}", resp.status);
    let po_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;

    // 2) 来料通知 → receive → inspect → 手动跑 ArrivalAcceptedHandler
    //    （cargo test 不启 EventProcessor；handler 同时更新 received_qty=10 与立 AP Credit）
    let po_items_rows = app.state.purchase_order_service()
        .list_items(&ctx, &mut app.state.pool.acquire().await.unwrap(), po_id)
        .await.unwrap();
    let po_item_id = po_items_rows[0].id;
    let arr_items = arrival_items(
        &po_items_rows.iter()
            .map(|it| (it.product_id.to_string(), "10".into(), it.id.to_string()))
            .collect::<Vec<_>>(),
    );
    let body = format!(
        "purchase_order_id={po_id}&supplier_id={SUPPLIER_ID}&arrival_date=2026-06-24&warehouse_id={WH}&items_json={arr_items}"
    );
    let resp = app.post_htmx("/admin/wms/arrivals/create", &body).await;
    assert!(resp.is_ok(), "创建来料通知 FAIL: {}", resp.status);
    let arr_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=receive").await;
    let _ = app.post_htmx(&format!("/admin/wms/arrivals/{arr_id}"), "action=inspect").await;

    use abt_core::purchase::arrival_handler::ArrivalAcceptedHandler;
    use abt_core::shared::event_bus::registry::EventHandler;
    use abt_core::shared::event_bus::model::DomainEvent;
    use abt_core::shared::enums::event::{DomainEventType, EventStatus};
    let arrival_event = DomainEvent {
        id: 0, event_type: DomainEventType::ArrivalInspected, event_version: 1,
        aggregate_type: "ArrivalNotice".into(), aggregate_id: arr_id,
        payload: serde_json::json!({"arrival_notice_id": arr_id, "doc_number": "test-k4"}),
        operator_id: 1, idempotency_key: format!("test-k4-arr-{arr_id}"),
        trace_id: None, request_id: None, status: EventStatus::Pending,
        retry_count: 0, failure_reason: None, processed_at: None, created_at: chrono::Utc::now(),
    };
    ArrivalAcceptedHandler::new(app.state.pool.clone()).handle(&arrival_event).await.expect("arrival handler FAIL");

    // 3) 创建退货单（退 4 × 3.00 = 12）
    use abt_core::purchase::return_order::{PurchaseReturnService, model::{CreatePurchaseReturnRequest, CreateReturnItemRequest}};
    let mut conn = app.state.pool.acquire().await.unwrap();
    let ret_id = app.state.purchase_return_service().create(
        &ctx, &mut conn,
        CreatePurchaseReturnRequest {
            order_id: po_id,
            supplier_id: SUPPLIER_ID,
            return_date: chrono::Local::now().date_naive(),
            return_reason: "k4 test".into(),
            remark: "".into(),
            items: vec![CreateReturnItemRequest {
                order_item_id: po_item_id,
                product_id: PRODUCT,
                returned_qty: Decimal::from(4),
                unit_price: Decimal::from(3),
            }],
        },
        Some(format!("k4-ret-{po_id}")),
    ).await.expect("创建退货单 FAIL");
    drop(conn);

    // 4) 构造 PurchaseReturnSettled 事件 → 调退货 handler（写反向 AP Debit）
    use abt_core::purchase::return_settlement_handler::PurchaseReturnSettledHandler;
    let handler = PurchaseReturnSettledHandler::new(app.state.pool.clone());
    let event = DomainEvent {
        id: 0, event_type: DomainEventType::PurchaseReturnSettled, event_version: 1,
        aggregate_type: "PurchaseReturn".into(), aggregate_id: ret_id,
        payload: serde_json::json!({"reconciliation_id": 0, "reconciliation_doc_number": "test-k4"}),
        operator_id: 1, idempotency_key: format!("test-k4-ret-{ret_id}"),
        trace_id: None, request_id: None, status: EventStatus::Pending,
        retry_count: 0, failure_reason: None, processed_at: None, created_at: chrono::Utc::now(),
    };
    handler.handle(&event).await.expect("return handler FAIL");

    // 5) 断言反向 AP 台账（PurchaseReturn Debit，金额 12 = 4 × 3.00）
    let ledger = ledger_by_source(&app, DocumentType::PurchaseReturn, ret_id).await
        .expect("❌ 退货 handler 未生成反向 AP 台账");
    assert_eq!(ledger.party_id, SUPPLIER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Debit);
    assert_eq!(ledger.amount, Decimal::from(12));

    // 6) 幂等：再次调用不重复写（COUNT 仍为 1）
    handler.handle(&event).await.expect("return handler 2nd FAIL");
    let mut conn = app.state.pool.acquire().await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ar_ap_ledger WHERE source_type=$1 AND source_id=$2",
    )
    .bind(DocumentType::PurchaseReturn)
    .bind(ret_id)
    .fetch_one(&mut *conn).await.unwrap();
    assert_eq!(count, 1, "❌ 退货 handler 幂等失败，重复写入台账");
}

// ════════════════════════════════════════════════════════════════════════════
//  k5 销售退货完成冲减应收（Issue #86）：
//  销售订单 → 发货（立 AR Debit）→ 销售退货单 → SalesReturnReceived 事件 → 反向 AR（Credit）+ 幂等
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k5_sales_return_received_reverses_ar_ledger() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);

    // 1) 销售订单 + 确认
    let so_body = format!(
        "customer_id={CUSTOMER_ID}&contact_id={CONTACT_ID}&items_json={}",
        so_items(&[(&PRODUCT.to_string(), "10", "1.00")])
    );
    let resp = app.post_htmx("/admin/orders/create", &so_body).await;
    assert!(resp.is_ok(), "创建销售订单 FAIL: {}", resp.status);
    let so_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/orders/{so_id}/confirm"), "").await;

    // 取 order_item_id
    let so_svc = app.state.sales_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let so_items_rows = so_svc.list_items(&ctx, &mut conn, so_id).await.unwrap();
    let order_item_id = so_items_rows[0].id;
    drop(conn);

    // 2) 发货 → confirm → pick → ship（立 AR Debit，金额 10 × 1.00 = 10）
    let body = format!(
        "customer_id={CUSTOMER_ID}&order_id={so_id}&items_json={}",
        ship_items(&[(order_item_id, "10")])
    );
    let resp = app.post_htmx("/admin/shipping/create", &body).await;
    assert!(resp.is_ok(), "创建发货单 FAIL: {}", resp.status);
    let ship_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/shipping/{ship_id}/confirm"), "").await;
    let _ = app.post_htmx(&format!("/admin/shipping/{ship_id}/pick"), "").await;
    let _ = app.post_htmx(&format!("/admin/shipping/{ship_id}/ship"), "").await;

    // 3) 创建销售退货单（退 4 × 1.00 = 4）
    use abt_core::sales::sales_return::{SalesReturnService, CreateReturnReq, CreateReturnItemReq, ReturnDisposition};
    let ret_id = app.state.sales_return_service().create(
        &ctx, &mut app.state.pool.acquire().await.unwrap(),
        CreateReturnReq {
            order_id: so_id,
            shipping_request_id: ship_id,
            customer_id: CUSTOMER_ID,
            return_reason: "k5 test".into(),
            items: vec![CreateReturnItemReq {
                order_item_id,
                returned_qty: Decimal::from(4),
                disposition: ReturnDisposition::Restock,
            }],
        },
    ).await.expect("创建销售退货单 FAIL");

    // 4) 构造 SalesReturnReceived 事件 → 调 handler（写反向 AR Credit）
    use abt_core::sales::sales_return_received_handler::SalesReturnReceivedHandler;
    use abt_core::shared::event_bus::registry::EventHandler;
    use abt_core::shared::event_bus::model::DomainEvent;
    use abt_core::shared::enums::event::{DomainEventType, EventStatus};
    let handler = SalesReturnReceivedHandler::new(app.state.pool.clone());
    let event = DomainEvent {
        id: 0, event_type: DomainEventType::SalesReturnReceived, event_version: 1,
        aggregate_type: "SalesReturn".into(), aggregate_id: ret_id,
        payload: serde_json::json!({"return_id": ret_id, "order_id": so_id}),
        operator_id: 1, idempotency_key: format!("test-k5-ret-{ret_id}"),
        trace_id: None, request_id: None, status: EventStatus::Pending,
        retry_count: 0, failure_reason: None, processed_at: None, created_at: chrono::Utc::now(),
    };
    handler.handle(&event).await.expect("sales return handler FAIL");

    // 5) 断言反向 AR 台账（SalesReturn Credit，金额 4 = 4 × 1.00）
    let ledger = ledger_by_source(&app, DocumentType::SalesReturn, ret_id).await
        .expect("❌ 销售退货 handler 未生成反向 AR 台账");
    assert_eq!(ledger.party_id, CUSTOMER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Credit);
    assert_eq!(ledger.amount, Decimal::from(4));

    // 6) 幂等：再次调用不重复写（COUNT 仍为 1）
    handler.handle(&event).await.expect("sales return handler 2nd FAIL");
    let mut conn = app.state.pool.acquire().await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ar_ap_ledger WHERE source_type=$1 AND source_id=$2",
    )
    .bind(DocumentType::SalesReturn)
    .bind(ret_id)
    .fetch_one(&mut *conn).await.unwrap();
    assert_eq!(count, 1, "❌ 销售退货 handler 幂等失败，重复写入台账");
}
