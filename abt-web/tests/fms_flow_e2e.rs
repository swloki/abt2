//! FMS 财务块全链路端到端测试
//!
//! 四条链路：
//! - k1 报销付款：expense.create → submit → approve → generate_payment_journal → CashJournal + Paid
//! - k2 收款核销：cash_journal.create(SalesReceipt) → confirm → write_off → 未核销余额
//! - k3 过度核销防护：write_off 累计 > source_total 应报 OverWriteOff
//! - k4 成本核算只读：cost_accounting 各查询 Ok

mod common;
use common::TestApp;

/// 生成唯一的 source_id（基于毫秒级时间戳 + 进程 id + prefix 哈希）
/// 确保测试可重复运行且跨多次运行不冲突（write_off 表按 source_id 聚合，
/// 若跨次跑 source_id 撞同一值会误判 OverWriteOff）。
/// source_id < 2^31 以适配 pg_advisory_xact_lock。
fn unique_source_id(prefix: &str) -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    // 毫秒时间戳取低 28 位（约 2.48 亿 ms ≈ 3 天循环，配合 pid + prefix 区分测试，
    // 跨多次跑冲突概率极低）
    let millis = (now.as_millis() as i64) & 0x0FFFFFFF;
    let pid = std::process::id() as i64 & 0xFFFF; // 16 位进程 id
    let prefix_hash = (prefix.chars().map(|c| c as i64).sum::<i64>() & 0xFF) << 4;
    // 组合：millis(28) | pid(16) | prefix(8+4) ≈ 48 位，仍 < 2^31
    // 实际取 millis 低 23 位 + pid 低 5 位 + prefix 哈希低 3 位 = 31 位
    let millis_low = millis & 0x7FFFFF; // 23 位
    let pid_low = (pid >> 3) & 0x1F; // 5 位
    let prefix_low = prefix_hash & 0x7; // 3 位
    // 保证为正：最高位 0
    millis_low | (pid_low << 23) | (prefix_low << 28)
}

use rust_decimal::Decimal;

/// 为 fms writeoff 测试创建独立客户，避免与 gl_invoice/gl_integration 测试共用客户 135
/// 导致的应收余额耦合（get_unreconciled_amount 虽按 source_id 聚合，但共用客户会让
/// 现金流/应收断言在 4-file 串行跑时偶发受 gl 测试数据污染）。
async fn create_isolated_customer(app: &TestApp, ctx: &ServiceContext, tag: &str) -> i64 {
    let mut conn = app.state.pool.acquire().await.unwrap();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    app.state
        .customer_service()
        .create(
            ctx,
            &mut conn,
            CreateCustomerReq {
                customer_name: format!("fms-writeoff-test-{tag}-{nanos}"),
                short_name: Some(tag.into()),
                category: CustomerCategory::DirectCustomer,
                industry: None,
                customer_level: None,
                region: None,
                tax_number: None,
                invoice_title: None,
                credit_limit: None,
                payment_terms: None,
                currency: Some("CNY".into()),
                receivable_account: Some("应收账款".into()),
                source: Some("e2e-test".into()),
                owner_id: None,
                remark: Some("fms writeoff e2e 隔离客户".into()),
            },
        )
        .await
        .expect("create isolated customer for fms writeoff e2e")
}

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
use abt_core::fms::cost_accounting::CostAccountingService;
use abt_core::master_data::customer::{
    model::{CreateCustomerReq, CustomerCategory},
    CustomerService,
};

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
                sheet_count: 1,
                has_invoice: true,
                attachments: vec![],
                items: vec![ExpenseItemInput {
                    expense_type: ExpenseType::Office,
                    amount: Decimal::from(120),
                    description: "办公用品".into(),
                    receipt_no: None,
                    cost_center: None,
                    profit_center: None,
                    occurrence_date: None,
                    has_invoice: true,
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

    // supervisor_approve: Submitted → SupervisorApproved
    svc.supervisor_approve(&ctx, &mut conn, id,
        abt_core::fms::expense::model::SupervisorApproveReq { remark: None },
    ).await.expect("supervisor_approve");
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::SupervisorApproved
    );

    // finance_approve: SupervisorApproved → FinanceApproved
    svc.finance_approve(&ctx, &mut conn, id,
        abt_core::fms::expense::model::FinanceApproveReq { remark: None },
    ).await.expect("finance_approve");
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::FinanceApproved
    );

    // approve (GM): FinanceApproved → Approved
    svc.approve(&ctx, &mut conn, id).await.expect("approve");
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::Approved
    );

    // pay: Approved → Paid + 建 CashJournal
    svc.pay(&ctx, &mut conn, id, abt_core::fms::expense::model::PayReq {
        payment_bank: "工商银行".into(),
        payment_remark: "e2e test payment".into(),
        payment_date: chrono::Utc::now().date_naive(),
    }).await.expect("pay");
    assert_eq!(
        svc.get(&ctx, &mut conn, id).await.unwrap().status,
        ExpenseStatus::Paid
    );

    // 断言付款日记账已生成
    let cj_svc = app.state.cash_journal_service();
    let recent = cj_svc.list_recent(&ctx, &mut conn, 1).await.unwrap();
    assert!(!recent.is_empty(), "cash journal should be created");
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
    // 用独立客户，不与 gl_invoice/gl_integration 共用客户 135
    let customer_id = create_isolated_customer(&app, &ctx, "k2").await;

    let cj_svc = app.state.cash_journal_service();
    let journal_id = cj_svc
        .create(
            &ctx,
            &mut conn,
            CreateCashJournalReq {
                journal_type: JournalType::SalesReceipt,
                direction: CashDirection::Inflow,
                amount,
                counterparty: CounterpartyRef::Customer(customer_id),
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
    // 用独立客户，不与 gl_invoice/gl_integration 共用客户 135
    let customer_id = create_isolated_customer(&app, &ctx, "k3").await;

    let cj_svc = app.state.cash_journal_service();
    let journal_id = cj_svc
        .create(
            &ctx,
            &mut conn,
            CreateCashJournalReq {
                journal_type: JournalType::SalesReceipt,
                direction: CashDirection::Inflow,
                amount,
                counterparty: CounterpartyRef::Customer(customer_id),
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

// ════════════════════════════════════════════════════════════════════════════
//  k4 成本核算只读：各查询返回 Ok（结果可空）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn k4_cost_accounting_queries_ok() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let svc = app.state.cost_accounting_service();

    let period = format!("{}", chrono::Utc::now().date_naive().format("%Y-%m"));

    // 各查询应 Ok（结果是否非空取决于 dev 库数据，不在此断言）
    let _ = svc.get_product_cost(&ctx, &mut conn, 565, period.clone()).await.unwrap();
    let _ = svc.list_product_costs(&mut conn, &period).await.unwrap();
    let _ = svc.list_work_order_costs(&mut conn).await.unwrap();
    let _ = svc.get_margin_analysis(&ctx, &mut conn, 1).await.unwrap();
}
