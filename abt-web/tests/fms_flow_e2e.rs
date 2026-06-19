//! FMS 财务块全链路端到端测试
//!
//! 四条链路：
//! - k1 报销付款：expense.create → submit → approve → generate_payment_journal → CashJournal + Paid
//! - k2 收款核销：cash_journal.create(SalesReceipt) → confirm → write_off → 未核销余额
//! - k3 过度核销防护：write_off 累计 > source_total 应报 OverWriteOff
//! - k4 成本核算只读：cost_accounting 各查询 Ok

mod common;
use common::TestApp;

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
