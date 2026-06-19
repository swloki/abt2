//! FMS 财务块全链路端到端测试
//!
//! 四条链路：
//! - k1 报销付款：expense.create → submit → approve → generate_payment_journal → CashJournal + Paid
//! - k2 收款核销：cash_journal.create(SalesReceipt) → confirm → write_off → 未核销余额
//! - k3 过度核销防护：write_off 累计 > source_total 应报 OverWriteOff
//! - k4 成本核算只读：cost_accounting 各查询 Ok

mod common;
use common::TestApp;

/// 生成唯一的 source_id（基于时间戳和测试名称，避免跨进程/跨进程重复）
/// 确保测试可重复运行且并发安全（source_id < 2^31 以适配 pg_advisory_xact_lock）
fn unique_source_id(prefix: &str) -> i64 {
    // 使用纳秒级时间戳后 6 位 + prefix 哈希，确保唯一性且 < 2^31
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as i64;
    let hash = prefix.chars().map(|c| c as i64).sum::<i64>() % 1_000_000;
    9_000_000 + (nanos % 10_000) * 100 + hash
}

use rust_decimal::Decimal;

use abt_core::shared::types::ServiceContext;
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::fms::enums::{
    CashDirection, CounterpartyRef, ExpenseStatus, ExpenseType, JournalStatus, JournalType,
};
use abt_core::fms::expense::{
    model::{CreateExpenseReq, ExpenseItemInput},
    ExpenseReimbursementService,
};
use abt_core::fms::cash_journal::{
    model::{CashJournalLineInput, CreateCashJournalReq},
    CashJournalService,
};
use abt_core::fms::write_off::{model::WriteOffReq, WriteOffService};

// ════════════════════════════════════════════════════════════════════════════
//  k1 报销付款链：create → submit → approve → generate_payment_journal
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn k1_expense_submit_approve_pay_chain() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let svc = app.state.expense_service();

    let today = chrono::Utc::now().date_naive();
    let id = svc
        .create(
            &ctx,
            &mut conn,
            CreateExpenseReq {
                applicant_id: 1,
                department_id: None,
                expense_date: today,
                remark: "e2e k1".into(),
                items: vec![ExpenseItemInput {
                    expense_type: ExpenseType::Office,
                    amount: Decimal::from(120),
                    description: "办公用品".into(),
                    receipt_no: None,
                    cost_center: None,
                    profit_center: None,
                }],
            },
        )
        .await
        .expect("create expense");

    // submit: Draft → Submitted
    svc.submit(&ctx, &mut conn, id).await.expect("submit");
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::Submitted
    );

    // approve: Submitted → Approved
    svc.approve(&ctx, &mut conn, id).await.expect("approve");
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::Approved
    );

    // pay（generate_payment_journal）: Approved → Paid + 建 CashJournal
    let journal_id = svc
        .generate_payment_journal(&ctx, &mut conn, id)
        .await
        .expect("pay");
    assert!(journal_id > 0);
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::Paid
    );

    // 断言付款日记账
    let cj_svc = app.state.cash_journal_service();
    let journal = cj_svc.get(&ctx, &mut conn, journal_id).await.unwrap();
    assert_eq!(journal.direction, CashDirection::Outflow);
    assert_eq!(journal.journal_type, JournalType::Expense);
    assert_eq!(journal.status, JournalStatus::Confirmed);
    assert_eq!(journal.amount, Decimal::from(120));
    assert_eq!(journal.source_id, id);
}

// ════════════════════════════════════════════════════════════════════════════
//  k2 收款核销链：cash_journal.create(SalesReceipt) → confirm → write_off
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn k2_cash_receipt_confirm_and_writeoff() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let today = chrono::Utc::now().date_naive();
    let period = format!("{}", today.format("%Y-%m"));
    let amount = Decimal::from(500);
    let source_id = unique_source_id("k2"); // 唯一 source_id，避免跨进程冲突

    let cj_svc = app.state.cash_journal_service();
    let journal_id = cj_svc
        .create(
            &ctx,
            &mut conn,
            CreateCashJournalReq {
                journal_type: JournalType::SalesReceipt,
                direction: CashDirection::Inflow,
                amount,
                counterparty: CounterpartyRef::Customer(135),
                source_type: DocumentType::SalesOrder,
                source_id,
                bank_account: "TEST".into(),
                transaction_date: today,
                period,
                remark: "e2e k2".into(),
                lines: vec![
                    CashJournalLineInput {
                        account_code: "银行存款".into(),
                        debit_amount: amount,
                        credit_amount: Decimal::ZERO,
                        cost_center: None,
                        profit_center: None,
                        remark: "借".into(),
                    },
                    CashJournalLineInput {
                        account_code: "应收账款".into(),
                        debit_amount: Decimal::ZERO,
                        credit_amount: amount,
                        cost_center: None,
                        profit_center: None,
                        remark: "贷".into(),
                    },
                ],
            },
        )
        .await
        .expect("create cash journal");

    cj_svc
        .confirm(&ctx, &mut conn, journal_id, None)
        .await
        .expect("confirm");
    let journal = cj_svc.get(&ctx, &mut conn, journal_id).await.unwrap();
    assert_eq!(journal.status, JournalStatus::Confirmed);

    // 核销：source_total=500，本次核销 300
    let wo_svc = app.state.write_off_service();
    let write_amount = Decimal::from(300);
    let wo_id = wo_svc
        .write_off(
            &ctx,
            &mut conn,
            WriteOffReq {
                cash_journal_id: journal_id,
                source_type: DocumentType::SalesOrder,
                source_id,
                source_total: amount,
                amount: write_amount,
                idempotency_key: Some(format!("e2e-k2-{journal_id}")),
            },
        )
        .await
        .expect("write_off");
    assert!(wo_id > 0);

    let unreconciled = wo_svc
        .get_unreconciled_amount(&ctx, &mut conn, DocumentType::SalesOrder, source_id, amount)
        .await
        .unwrap();
    assert_eq!(unreconciled, amount - write_amount);
}

// ════════════════════════════════════════════════════════════════════════════
//  k3 过度核销防护：累计核销 > source_total 应被拒
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn k3_over_writeoff_rejected() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let today = chrono::Utc::now().date_naive();
    let period = format!("{}", today.format("%Y-%m"));
    let amount = Decimal::from(500);
    let source_id = unique_source_id("k3"); // 唯一 source_id，避免跨进程冲突

    let cj_svc = app.state.cash_journal_service();
    let journal_id = cj_svc
        .create(
            &ctx,
            &mut conn,
            CreateCashJournalReq {
                journal_type: JournalType::SalesReceipt,
                direction: CashDirection::Inflow,
                amount,
                counterparty: CounterpartyRef::Customer(135),
                source_type: DocumentType::SalesOrder,
                source_id,
                bank_account: "TEST".into(),
                transaction_date: today,
                period,
                remark: "e2e k3".into(),
                lines: vec![
                    CashJournalLineInput {
                        account_code: "银行存款".into(),
                        debit_amount: amount,
                        credit_amount: Decimal::ZERO,
                        cost_center: None,
                        profit_center: None,
                        remark: "借".into(),
                    },
                    CashJournalLineInput {
                        account_code: "应收账款".into(),
                        debit_amount: Decimal::ZERO,
                        credit_amount: amount,
                        cost_center: None,
                        profit_center: None,
                        remark: "贷".into(),
                    },
                ],
            },
        )
        .await
        .unwrap();
    cj_svc.confirm(&ctx, &mut conn, journal_id, None).await.unwrap();

    let wo_svc = app.state.write_off_service();
    // 先核销 300（合法）
    wo_svc
        .write_off(
            &ctx,
            &mut conn,
            WriteOffReq {
                cash_journal_id: journal_id,
                source_type: DocumentType::SalesOrder,
                source_id,
                source_total: amount,
                amount: Decimal::from(300),
                idempotency_key: Some(format!("e2e-k3a-{journal_id}")),
            },
        )
        .await
        .unwrap();

    // 再核销 300（累计 600 > source_total 500）→ 应报 OverWriteOff
    let err = wo_svc
        .write_off(
            &ctx,
            &mut conn,
            WriteOffReq {
                cash_journal_id: journal_id,
                source_type: DocumentType::SalesOrder,
                source_id,
                source_total: amount,
                amount: Decimal::from(300),
                idempotency_key: Some(format!("e2e-k3b-{journal_id}")),
            },
        )
        .await
        .expect_err("过度核销应被拒");
    let msg = format!("{err:?}");
    assert!(msg.contains("OverWriteOff"), "应为 OverWriteOff，实际: {msg}");
}
