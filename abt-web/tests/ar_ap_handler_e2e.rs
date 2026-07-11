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
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::om::outsourcing_order::OutsourcingOrderService;

// ── 测试数据常量（dev 库实测可用，与 sales_to_wms_e2e 一致）──
const CUSTOMER_ID: i64 = 135;
const CONTACT_ID: i64 = 135;
const SUPPLIER_ID: i64 = 129;
const WH: i64 = 23320; // 备料周转仓
#[allow(dead_code)] // 仅测试辅助 fn stock_in 引用
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

fn redirect_id(resp: &common::TestResponse) -> i64 {
    let loc = resp.hx_redirect().unwrap_or("");
    loc.rsplit('/').next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// 直接入库备料（绕过 web 表单，确保发货有库存）
#[allow(dead_code)]
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
    sqlx::query_as::<_, abt_core::fms::ar_ap::model::ArApLedger>(
        "SELECT * FROM ar_ap_ledger WHERE source_type = $1 AND source_id = $2 ORDER BY id DESC LIMIT 1",
    )
    .bind(source_type)
    .bind(source_id)
    .fetch_optional(&mut *conn)
    .await
    .unwrap()
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
    let resp = app.post_htmx("/admin/wms/shipping/create", &body).await;
    assert!(resp.is_ok(), "创建发货单 FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());
    let ship_id = redirect_id(&resp);
    assert!(ship_id > 0, "应返回发货单 id");

    let _ = app.post_htmx(&format!("/admin/wms/shipping/{ship_id}/confirm"), "").await;
    let _ = app.post_htmx(&format!("/admin/wms/shipping/{ship_id}/pick"), "").await;
    let resp = app.post_htmx(&format!("/admin/wms/shipping/{ship_id}/ship"), "").await;
    assert!(resp.is_ok() || resp.is_redirect(), "发货 ship FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    // 4) 验证 AR 台账（ShippingRequest Debit 应收，金额 = 10 × 1.00 = 10）
    let ledger = ledger_by_source(&app, DocumentType::ShippingRequest, ship_id).await;
    let ledger = ledger.expect("❌ ship 未生成 AR 台账");
    assert_eq!(ledger.party_id, CUSTOMER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Debit);
    assert_eq!(ledger.amount, Decimal::from(10));
}

// ════════════════════════════════════════════════════════════════════════════
//  k2 采购到财务：采购订单 → PO 直收入库（receive_purchase）→ AP 台账（Credit 应付）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k2_purchase_arrival_to_ap_ledger() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);

    // 1) 采购订单 → submit
    let items = po_items(&[(&PRODUCT.to_string(), "k2-PO", "20", "2.00")]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-23&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "创建采购订单 FAIL: {}", resp.status);
    let po_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;

    // 2) PO 直收入库（取消来料通知后）：调 PurchaseStockInService 立 AP Credit
    let mut conn = app.state.pool.acquire().await.unwrap();
    let po_items_rows = app.state.purchase_order_service().list_items(&ctx, &mut conn, po_id).await.unwrap();
    let order_item_id = po_items_rows[0].id;
    drop(conn);

    use abt_core::wms::picking::{model::{PoReceiveRow, ReceivePurchaseReq}, PickingService};
    let req = ReceivePurchaseReq {
        po_id,
        rows: vec![PoReceiveRow {
            order_item_id, product_id: PRODUCT, received_qty: Decimal::from(20),
            batch_no: None, warehouse_id: WH, bin_id: Some(BIN),
        }],
        delivery_note: None, remark: None,
        idempotency_key: Some(format!("test-k2-{po_id}")),
    };
    let mut tx = app.state.pool.begin().await.unwrap();
    app.state.picking_service().receive_purchase(&ctx, &mut tx, req).await.expect("receive FAIL");
    tx.commit().await.unwrap();

    // 3) 验证 AP 台账（PurchaseOrder Credit，金额 40 = 20 × 2.00）
    let ledger = ledger_by_source(&app, DocumentType::PurchaseOrder, po_id).await
        .expect("❌ receive_purchase 未生成 AP 台账");
    assert_eq!(ledger.party_id, SUPPLIER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Credit);
    assert_eq!(ledger.amount, Decimal::from(40)); // 20 × 2.00
}

// ════════════════════════════════════════════════════════════════════════════
//  k8 采购 PO 直收入库（取消来料通知后）：PO → receive_purchase → 入库/回写PO/立应付/成本
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k8_purchase_po_direct_stock_in() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);

    // 1) 采购订单 → 提交
    let items = po_items(&[(&PRODUCT.to_string(), "k8-PO", "20", "2.00")]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-26&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "创建采购订单 FAIL: {}", resp.status);
    let po_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;

    // 2) 取 PO 明细 order_item_id
    let mut conn = app.state.pool.acquire().await.unwrap();
    let po_items_rows = app.state.purchase_order_service().list_items(&ctx, &mut conn, po_id).await.unwrap();
    let order_item_id = po_items_rows[0].id;
    drop(conn);

    // 3) 直接调 PurchaseStockInService（test handle，事务包裹）—— PO 直收入库闭环
    use abt_core::wms::picking::{model::{PoReceiveRow, ReceivePurchaseReq}, PickingService};
    let req = ReceivePurchaseReq {
        po_id,
        rows: vec![PoReceiveRow {
            order_item_id,
            product_id: PRODUCT,
            received_qty: Decimal::from(20),
            batch_no: None,
            warehouse_id: WH,
            bin_id: Some(BIN),
        }],
        delivery_note: None,
        remark: None,
        idempotency_key: Some(format!("test-k8-{po_id}")),
    };
    let mut tx = app.state.pool.begin().await.unwrap();
    app.state.picking_service()
        .receive_purchase(&ctx, &mut tx, req)
        .await
        .expect("receive_purchase FAIL");
    tx.commit().await.unwrap();

    // 4) 验证 PO received_qty 回写 + 状态流转 Received
    let mut conn = app.state.pool.acquire().await.unwrap();
    let po_items_after = app.state.purchase_order_service().list_items(&ctx, &mut conn, po_id).await.unwrap();
    assert_eq!(po_items_after[0].received_qty, Decimal::from(20), "❌ PO received_qty 未回写");
    let po = app.state.purchase_order_service().get(&ctx, &mut conn, po_id).await.unwrap();
    use abt_core::purchase::enums::PurchaseOrderStatus;
    assert_eq!(po.status, PurchaseOrderStatus::Received, "❌ PO 状态未流转到 Received");

    // 5) 验证库存流水 source_type=purchase_order
    let txn: Option<(String,)> = sqlx::query_as(
        "SELECT source_type FROM inventory_transactions WHERE source_id=$1 AND source_type='purchase_order' ORDER BY id DESC LIMIT 1",
    )
    .bind(po_id)
    .fetch_optional(&mut *conn)
    .await
    .unwrap();
    drop(conn);
    assert!(txn.is_some(), "❌ 库存流水 source_type 应为 purchase_order");

    // 6) 验证 AP 台账（PurchaseOrder Credit，金额 = 20 × 2.00 = 40）
    let ledger = ledger_by_source(&app, DocumentType::PurchaseOrder, po_id).await
        .expect("❌ receive_purchase 未生成 AP 台账");
    assert_eq!(ledger.party_id, SUPPLIER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Credit);
    assert_eq!(ledger.amount, Decimal::from(40));
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

    // 2) PO 直收入库（取消来料通知后）：调 PurchaseStockInService，立 AP Credit + 回写 received_qty=10
    let po_items_rows = app.state.purchase_order_service()
        .list_items(&ctx, &mut app.state.pool.acquire().await.unwrap(), po_id)
        .await.unwrap();
    let po_item_id = po_items_rows[0].id;

    use abt_core::wms::picking::{model::{PoReceiveRow, ReceivePurchaseReq}, PickingService};
    let req = ReceivePurchaseReq {
        po_id,
        rows: vec![PoReceiveRow {
            order_item_id: po_item_id, product_id: PRODUCT, received_qty: Decimal::from(10),
            batch_no: None, warehouse_id: WH, bin_id: Some(BIN),
        }],
        delivery_note: None, remark: None,
        idempotency_key: Some(format!("test-k4-{po_id}")),
    };
    let mut tx = app.state.pool.begin().await.unwrap();
    app.state.picking_service().receive_purchase(&ctx, &mut tx, req).await.expect("receive FAIL");
    tx.commit().await.unwrap();

    use abt_core::shared::event_bus::registry::EventHandler;
    use abt_core::shared::event_bus::model::DomainEvent;
    use abt_core::shared::enums::event::{DomainEventType, EventStatus};

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
    let resp = app.post_htmx("/admin/wms/shipping/create", &body).await;
    assert!(resp.is_ok(), "创建发货单 FAIL: {}", resp.status);
    let ship_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/wms/shipping/{ship_id}/confirm"), "").await;
    let _ = app.post_htmx(&format!("/admin/wms/shipping/{ship_id}/pick"), "").await;
    let _ = app.post_htmx(&format!("/admin/wms/shipping/{ship_id}/ship"), "").await;

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

// ════════════════════════════════════════════════════════════════════════════
//  k6 采购入库统一入口（治本）：库存入库页选采购单 → 自动建来料通知+收货+检验+入库
//  验证 create_stock_in(source_type=purchase) 编排来料通知闭环：
//  PO received_qty/状态回写 + AP台账(Credit) + 库存流水(source=arrival_notice) + 单据关联
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k6_stock_in_purchase_unified_closed_loop() {
    let app = TestApp::new().await;

    // 1) 采购订单 → submit（PRODUCT 565, qty=12, price=2.50）
    let items = po_items(&[(&PRODUCT.to_string(), "k6-PO", "12", "2.50")]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-25&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "创建采购订单 FAIL: {}", resp.status);
    let po_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;

    // 查 PO 单号 + 565 独占的 bin（该 bin 无其他产品占用，满足「一库位一产品」规则）
    let mut conn = app.state.pool.acquire().await.unwrap();
    let po_doc: String = sqlx::query_scalar("SELECT doc_number FROM purchase_orders WHERE id=$1")
        .bind(po_id).fetch_one(&mut *conn).await.unwrap();
    let bin_id: i64 = sqlx::query_scalar(
        "SELECT s.bin_id FROM stock_ledger s \
         WHERE s.product_id=$1 AND s.quantity>0 AND s.warehouse_id=$2 \
         AND NOT EXISTS (SELECT 1 FROM stock_ledger s2 WHERE s2.bin_id=s.bin_id AND s2.product_id<>s.product_id AND s2.quantity>0) \
         ORDER BY s.id LIMIT 1",
    ).bind(PRODUCT).bind(WH).fetch_one(&mut *conn).await
        .expect("❌ PRODUCT 在 WH 无独占 bin（需预置）");
    drop(conn);

    // 2) 库存入库页选采购单 → create_stock_in（source_type=purchase）
    //    items_json: product_id/quantity/warehouse_id/bin_id/source_id(PO)/source_doc_number(PO单号)
    let stockin_items = urlenc(&format!(
        r#"[{{"product_id":"{PRODUCT}","quantity":"12","warehouse_id":"{WH}","bin_id":"{bin_id}","source_id":"{po_id}","source_doc_number":"{po_doc}"}}]"#
    ));
    let body = format!(
        "transaction_type=PurchaseReceipt&source_type=purchase&source_ref={po_doc}&idempotency_key=k6-{po_id}&items_json={stockin_items}"
    );
    let resp = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "create_stock_in FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(500).collect::<String>()
    );

    // 3) create_stock_in 走 PurchaseStockInService 直收入库闭环（事务内 record 库存 + 回写 PO +
    //    立应付 + 成本）。取消来料通知后不再建来料通知、不依赖 ArrivalAcceptedHandler（同步完成）。

    // 4) PO 回写：received_qty=12, status=Received(4)
    let mut conn = app.state.pool.acquire().await.unwrap();
    let (po_status, received_qty): (i16, Decimal) = sqlx::query_as(
        "SELECT po.status, poi.received_qty FROM purchase_orders po \
         JOIN purchase_order_items poi ON poi.order_id=po.id WHERE po.id=$1",
    ).bind(po_id).fetch_one(&mut *conn).await.unwrap();
    assert_eq!(po_status, 4, "❌ PO 状态未推进到 Received(4)");
    assert_eq!(received_qty, Decimal::from(12), "❌ PO received_qty 未回写为 12");

    // 5) 库存流水 source_type=purchase_order, source_id=po_id（治本核心：库存关联 PO）
    let txn: (String, i64) = sqlx::query_as(
        "SELECT source_type, source_id FROM inventory_transactions \
         WHERE product_id=$1 AND source_type='purchase_order' AND source_id=$2 ORDER BY id DESC LIMIT 1",
    ).bind(PRODUCT).bind(po_id).fetch_one(&mut *conn).await
        .expect("❌ 未找到关联 PO 的库存流水");
    assert_eq!(txn.0, "purchase_order", "❌ 库存流水 source_type 应为 purchase_order");
    drop(conn);

    // 6) AP 台账（PurchaseOrder Credit，金额 30 = 12 × 2.50，party=供应商）
    let ledger = ledger_by_source(&app, DocumentType::PurchaseOrder, po_id).await
        .expect("❌ 未生成 AP 台账");
    assert_eq!(ledger.party_id, SUPPLIER_ID);
    assert_eq!(ledger.direction, abt_core::fms::ar_ap::enums::LedgerDirection::Credit);
    assert_eq!(ledger.amount, Decimal::from(30));
}

// ════════════════════════════════════════════════════════════════════════════
//  k7 采购入库幂等：同一 idempotency_key 重复提交只入库一次（防双击/网络重试）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k7_stock_in_idempotency() {
    let app = TestApp::new().await;

    // 1) 采购订单 → submit
    let items = po_items(&[(&PRODUCT.to_string(), "k7-PO", "5", "1.00")]);
    let body = format!("supplier_id={SUPPLIER_ID}&order_date=2026-06-25&items_json={items}&currency=CNY");
    let resp = app.post_htmx("/admin/purchase/orders/create", &body).await;
    assert!(resp.is_ok(), "创建采购订单 FAIL: {}", resp.status);
    let po_id = redirect_id(&resp);
    let _ = app.post_htmx(&format!("/admin/purchase/orders/{po_id}/submit"), "").await;

    // 565 独占 bin（满足一库位一产品）
    let mut conn = app.state.pool.acquire().await.unwrap();
    let bin_id: i64 = sqlx::query_scalar(
        "SELECT s.bin_id FROM stock_ledger s \
         WHERE s.product_id=$1 AND s.quantity>0 AND s.warehouse_id=$2 \
         AND NOT EXISTS (SELECT 1 FROM stock_ledger s2 WHERE s2.bin_id=s.bin_id AND s2.product_id<>s.product_id AND s2.quantity>0) \
         ORDER BY s.id LIMIT 1",
    ).bind(PRODUCT).bind(WH).fetch_one(&mut *conn).await.unwrap();
    drop(conn);

    // 入库 form（固定 idempotency_key，模拟前端生成的同一 key 被提交两次）
    let stockin_items = urlenc(&format!(
        r#"[{{"product_id":"{PRODUCT}","quantity":"5","warehouse_id":"{WH}","bin_id":"{bin_id}","source_id":"{po_id}","source_doc_number":""}}]"#
    ));
    let body = format!(
        "transaction_type=PurchaseReceipt&source_type=purchase&idempotency_key=k7-dup-{po_id}&items_json={stockin_items}"
    );

    // 2) 第一次提交 → 成功（走 PurchaseStockInService 直收入库）
    let resp1 = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(resp1.is_ok() || resp1.is_redirect(), "第一次提交 FAIL: {}", resp1.status);

    let mut conn = app.state.pool.acquire().await.unwrap();
    let recv_1: Decimal = sqlx::query_scalar(
        "SELECT poi.received_qty FROM purchase_order_items poi WHERE poi.order_id=$1",
    ).bind(po_id).fetch_one(&mut *conn).await.unwrap();
    drop(conn);
    assert_eq!(recv_1, Decimal::from(5), "第一次提交后 received_qty 应为 5");

    // 3) 第二次提交（同 idempotency_key）→ 幂等跳过
    let resp2 = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(resp2.is_ok() || resp2.is_redirect(), "第二次提交应幂等返回成功，实际 {}", resp2.status);

    // 4) received_qty 不变（幂等：第二次没重复累加）
    let mut conn = app.state.pool.acquire().await.unwrap();
    let recv_2: Decimal = sqlx::query_scalar(
        "SELECT poi.received_qty FROM purchase_order_items poi WHERE poi.order_id=$1",
    ).bind(po_id).fetch_one(&mut *conn).await.unwrap();
    drop(conn);
    assert_eq!(recv_2, Decimal::from(5), "❌ 幂等失败：第二次提交重复累加 received_qty（{}）", recv_2);
}

// ════════════════════════════════════════════════════════════════════════════
//  k5 一键申请发货（销售订单详情页申请 → 仓库 work-center 待发货）
//  SO 确认 → POST /admin/orders/{id}/request-ship（销售不选仓库）→ 订单 ShippingRequested(8)
//  + 发货单 Confirmed(2)（跳过 Draft，直接进 work-center 待发货）+ 明细 warehouse_id NULL
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn k5_one_click_request_ship_to_work_center() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);

    // 1) SO + confirm
    let so_body = format!(
        "customer_id={CUSTOMER_ID}&contact_id={CONTACT_ID}&items_json={}",
        so_items(&[(&PRODUCT.to_string(), "5", "1.00")])
    );
    let resp = app.post_htmx("/admin/orders/create", &so_body).await;
    assert!(resp.is_ok(), "创建销售订单 FAIL: {}", resp.status);
    let so_id = redirect_id(&resp);
    assert!(so_id > 0, "应返回 SO id");
    let _ = app.post_htmx(&format!("/admin/orders/{so_id}/confirm"), "").await;

    // 取 order_item_id
    let so_svc = app.state.sales_order_service();
    let mut conn = app.state.pool.acquire().await.unwrap();
    let order_item_id = so_svc.list_items(&ctx, &mut conn, so_id).await.unwrap()[0].id;
    drop(conn);

    // 2) 一键申请发货（items_json 不含 warehouse_id，销售不选仓库）
    let items_json = urlenc(&format!(
        r#"[{{"order_item_id":{order_item_id},"requested_qty":"5"}}]"#
    ));
    let body = format!("items_json={items_json}");
    let resp = app.post_htmx(&format!("/admin/orders/{so_id}/request-ship"), &body).await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "申请发货 FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );

    // 3) 订单 status=8（ShippingRequested）
    let mut conn = app.state.pool.acquire().await.unwrap();
    let so_status: i16 = sqlx::query_scalar("SELECT status FROM sales_orders WHERE id=$1")
        .bind(so_id).fetch_one(&mut *conn).await.unwrap();
    assert_eq!(so_status, 8, "❌ 订单应推进到 ShippingRequested(8)，实际 {so_status}");

    // 4) 发货 picking status=2（Confirmed，跳过 Draft，直接进 work-center 待发货）
    let ship_status: i16 = sqlx::query_scalar(
        "SELECT status FROM stock_pickings WHERE picking_type=3 AND source_id=$1 ORDER BY id DESC LIMIT 1",
    ).bind(so_id).fetch_one(&mut *conn).await.unwrap();
    assert_eq!(ship_status, 2, "❌ 发货 picking 应为 Confirmed(2)（跳过 Draft）");

    // 5) 发货明细 from_warehouse_id NULL（销售不指定仓库，direct_ship 时选）
    let ship_id: i64 = sqlx::query_scalar(
        "SELECT id FROM stock_pickings WHERE picking_type=3 AND source_id=$1 ORDER BY id DESC LIMIT 1",
    ).bind(so_id).fetch_one(&mut *conn).await.unwrap();
    let wh_id: Option<i64> = sqlx::query_scalar(
        "SELECT from_warehouse_id FROM stock_picking_items WHERE picking_id=$1 LIMIT 1",
    ).bind(ship_id).fetch_one(&mut *conn).await.unwrap();
    assert!(wh_id.is_none(), "❌ 发货明细仓库应为 NULL（销售不指定仓库），实际 {:?}", wh_id);
}
