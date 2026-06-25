//! Phase 2 砍 GL/发票后的往来台账 + 核销闭环 e2e：
//! - 直接 insert AR/AP 台账（不经发票实体，业务单据直接立账）
//! - settle 核销（CashJournal 收/付款 ↔ ShippingRequest/ArrivalNotice 应收应付）
//! - amount_applied 累加验证

mod common;
use common::TestApp;
use serial_test::serial;

use rust_decimal::Decimal;
use abt_core::shared::types::ServiceContext;
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use abt_core::fms::ar_ap::model::SettleReq;
use abt_core::fms::ar_ap::service::ArApService;
use abt_core::fms::ar_ap::enums::LedgerDirection;
use abt_core::fms::enums::CounterpartyType;

async fn customer_id() -> i64 { 135 }
async fn supplier_id() -> i64 { 129 }

// 用微秒时间戳生成强唯一 source_id，避免与历史/其他测试数据冲突
// （#89 加 partial UNIQUE 后，确定性 id 会与 dev 库历史残留冲突）
fn uid(seed: i64) -> i64 {
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;
    micros.wrapping_add(seed)
}

// ════════════════════════════════════════════════════════════════════════════
//  k1 AR 台账 + 收款核销闭环（发货 Debit → 收款 Credit → settle）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn k1_ar_ledger_settle_cycle() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let svc = app.state.ar_ap_service();

    let ar_src = uid(1);
    let pay_src = uid(2);
    let today = chrono::Utc::now().date_naive();

    // 直接立 AR 台账（发货 Debit 应收 1000）
    let ar_id = ArApLedgerRepo::insert(
        &mut conn,
        &ArApLedgerInsert {
            party_type: CounterpartyType::Customer,
            party_id: customer_id().await,
            source_type: DocumentType::ShippingRequest,
            source_id: ar_src,
            source_doc_no: "TEST-SO-001",
            against_type: None,
            against_id: None,
            direction: LedgerDirection::Debit,
            amount: Decimal::from(1000),
            currency: "CNY",
            exchange_rate: Decimal::ONE,
            transaction_date: today,
            due_date: None,
            period: "2026-06",
            description: "test AR",
            operator_id: 1,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // 立收款台账（CashJournal Credit 600，部分核销）
    ArApLedgerRepo::insert(
        &mut conn,
        &ArApLedgerInsert {
            party_type: CounterpartyType::Customer,
            party_id: customer_id().await,
            source_type: DocumentType::CashJournal,
            source_id: pay_src,
            source_doc_no: "TEST-PAY-001",
            against_type: Some(DocumentType::ShippingRequest),
            against_id: Some(ar_src),
            direction: LedgerDirection::Credit,
            amount: Decimal::from(600),
            currency: "CNY",
            exchange_rate: Decimal::ONE,
            transaction_date: today,
            due_date: None,
            period: "2026-06",
            description: "test payment",
            operator_id: 1,
        },
    )
    .await
    .unwrap();

    // settle 核销 600
    svc.settle(
        &ctx,
        &mut conn,
        SettleReq {
            payment_source_type: DocumentType::CashJournal,
            payment_source_id: pay_src,
            invoice_source_type: DocumentType::ShippingRequest,
            invoice_source_id: ar_src,
            amount: Decimal::from(600),
        },
    )
    .await
    .unwrap();

    // 验证：AR 台账 amount_applied=600，未清 400，未全额核销
    let ar = ArApLedgerRepo::get_by_id(&mut conn, ar_id).await.unwrap().unwrap();
    assert_eq!(ar.amount_applied, Decimal::from(600));
    assert_eq!(ar.outstanding(), Decimal::from(400));
    assert!(!ar.is_fully_settled());
}

// ════════════════════════════════════════════════════════════════════════════
//  k2 AP 台账 + 付款核销闭环（入库 Credit → 付款 Debit → settle 全额）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn k2_ap_ledger_settle_cycle() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let svc = app.state.ar_ap_service();

    let ap_src = uid(3);
    let pay_src = uid(4);
    let today = chrono::Utc::now().date_naive();

    // 直接立 AP 台账（入库 Credit 应付 400）
    let ap_id = ArApLedgerRepo::insert(
        &mut conn,
        &ArApLedgerInsert {
            party_type: CounterpartyType::Supplier,
            party_id: supplier_id().await,
            source_type: DocumentType::PurchaseOrder,
            source_id: ap_src,
            source_doc_no: "TEST-AN-001",
            against_type: None,
            against_id: None,
            direction: LedgerDirection::Credit,
            amount: Decimal::from(400),
            currency: "CNY",
            exchange_rate: Decimal::ONE,
            transaction_date: today,
            due_date: None,
            period: "2026-06",
            description: "test AP",
            operator_id: 1,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // 立付款台账（CashJournal Debit 400，全额核销）
    ArApLedgerRepo::insert(
        &mut conn,
        &ArApLedgerInsert {
            party_type: CounterpartyType::Supplier,
            party_id: supplier_id().await,
            source_type: DocumentType::CashJournal,
            source_id: pay_src,
            source_doc_no: "TEST-PAY-002",
            against_type: Some(DocumentType::PurchaseOrder),
            against_id: Some(ap_src),
            direction: LedgerDirection::Debit,
            amount: Decimal::from(400),
            currency: "CNY",
            exchange_rate: Decimal::ONE,
            transaction_date: today,
            due_date: None,
            period: "2026-06",
            description: "test payment",
            operator_id: 1,
        },
    )
    .await
    .unwrap();

    // settle 全额核销 400
    svc.settle(
        &ctx,
        &mut conn,
        SettleReq {
            payment_source_type: DocumentType::CashJournal,
            payment_source_id: pay_src,
            invoice_source_type: DocumentType::PurchaseOrder,
            invoice_source_id: ap_src,
            amount: Decimal::from(400),
        },
    )
    .await
    .unwrap();

    // 验证：AP 台账全额核销
    let ap = ArApLedgerRepo::get_by_id(&mut conn, ap_id).await.unwrap().unwrap();
    assert_eq!(ap.amount_applied, Decimal::from(400));
    assert_eq!(ap.outstanding(), Decimal::ZERO);
    assert!(ap.is_fully_settled());
}
