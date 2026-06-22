//! 业财衔接 e2e：收付款/报销付款过账 → 验证 GL 凭证 + 科目余额
//!
//! 验证 Plan C1/C2 引入的业财一体过账：
//! - k1 SalesReceipt 收款：cash_journal.confirm → 借 default_bank / 贷 default_ar
//!   - 银行（借方科目）余额 +1000；应收（借方科目）余额 -1000（收款核销应收）
//! - k2 PurchasePayment 付款：cash_journal.confirm → 借 default_ap / 贷 default_bank
//!   - 应付（贷方科目）余额 -1000（付款冲销应付）；银行（借方科目）余额 -1000
//! - k3 报销付款：expense.generate_payment_journal → 借 default_expense / 贷 default_bank
//!   - 费用（借方科目）余额 +amount；银行（借方科目）余额 -amount
//!
//! 所有用例 #[serial] 串行：共享 dev DB，并行会污染余额快照。

mod common;
use common::TestApp;
use serial_test::serial;

use rust_decimal::Decimal;

use abt_core::fms::cash_journal::{
    model::{CashJournalLineInput, CreateCashJournalReq},
    CashJournalService,
};
use abt_core::fms::enums::{CashDirection, CounterpartyRef, ExpenseType, JournalType};
use abt_core::fms::expense::{
    model::{CreateExpenseReq, ExpenseItemInput},
    ExpenseReimbursementService,
};
use abt_core::gl::entry::GlEntryService;
use abt_core::gl::mapping::GlMappingService;
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::types::ServiceContext;

// ════════════════════════════════════════════════════════════════════════════
//  Helper: dev 库固定 ID（与 fms_flow_e2e / gl_invoice_flow_e2e 一致）
// ════════════════════════════════════════════════════════════════════════════

/// dev 库固定客户 id（与 gl_invoice_flow_e2e::customer_id 一致）
fn customer_id() -> i64 {
    135
}
/// dev 库固定供应商 id（与 gl_invoice_flow_e2e::supplier_id 一致）
fn supplier_id() -> i64 {
    129
}
/// 报销申请人 = 员工 id（与 fms_flow_e2e k1 的 applicant_id 一致）
fn employee_id() -> i64 {
    1
}

/// 生成唯一 source_id（< 2^31，适配 pg_advisory_xact_lock；与 fms_flow_e2e 同款）
fn unique_source_id(prefix: &str) -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as i64;
    let hash = prefix.chars().map(|c| c as i64).sum::<i64>() % 1_000_000;
    9_100_000 + (nanos % 10_000) * 100 + hash
}

// ════════════════════════════════════════════════════════════════════════════
//  k1 SalesReceipt 收款过账：借 default_bank / 贷 default_ar
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn k1_sales_receipt_confirm_posts_gl() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let cj = app.state.cash_journal_service();
    let entry = app.state.gl_entry_service();
    let map = app.state.gl_mapping_service();

    let bank_acct = map
        .resolve(&ctx, &mut conn, "default_bank", None)
        .await
        .expect("resolve default_bank");
    let ar_acct = map
        .resolve(&ctx, &mut conn, "default_ar", None)
        .await
        .expect("resolve default_ar");

    // 记录初始余额
    let bank_before = entry
        .get_account_balance(&ctx, &mut conn, bank_acct, None, None)
        .await
        .unwrap();
    let ar_before = entry
        .get_account_balance(&ctx, &mut conn, ar_acct, None, None)
        .await
        .unwrap();

    let today = chrono::Utc::now().date_naive();
    let period = format!("{}", today.format("%Y-%m"));
    let amount = Decimal::from(1000);
    let source_id = unique_source_id("k1gl");

    // 建 SalesReceipt CJ（借银行 1000 / 贷应收 1000，明细平衡）
    let id = cj
        .create(
            &ctx,
            &mut conn,
            CreateCashJournalReq {
                journal_type: JournalType::SalesReceipt,
                direction: CashDirection::Inflow,
                amount,
                counterparty: CounterpartyRef::Customer(customer_id()),
                source_type: DocumentType::SalesOrder,
                source_id,
                bank_account: "TEST".into(),
                transaction_date: today,
                period,
                remark: "e2e k1 收款过账".into(),
                lines: vec![
                    CashJournalLineInput {
                        account_code: "银行存款".into(),
                        debit_amount: amount,
                        credit_amount: Decimal::ZERO,
                        cost_center: None,
                        profit_center: None,
                        remark: "借银行".into(),
                    },
                    CashJournalLineInput {
                        account_code: "应收账款".into(),
                        debit_amount: Decimal::ZERO,
                        credit_amount: amount,
                        cost_center: None,
                        profit_center: None,
                        remark: "贷应收".into(),
                    },
                ],
            },
        )
        .await
        .expect("create cash journal");

    // confirm → 同事务过账 GL（借 default_bank / 贷 default_ar）
    cj.confirm(&ctx, &mut conn, id, None)
        .await
        .expect("confirm posts GL");

    // 银行（借方科目）：debit+1000 → 余额 +1000
    let bank_after = entry
        .get_account_balance(&ctx, &mut conn, bank_acct, None, None)
        .await
        .unwrap();
    assert_eq!(
        bank_after - bank_before,
        amount,
        "收款 confirm 后银行余额应增加 {amount}"
    );

    // 应收（借方科目）：credit+1000 → 余额 -1000（收款冲销应收）
    let ar_after = entry
        .get_account_balance(&ctx, &mut conn, ar_acct, None, None)
        .await
        .unwrap();
    assert_eq!(
        ar_before - ar_after,
        amount,
        "收款 confirm 后应收余额应减少 {amount}（贷方冲销）"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  k2 PurchasePayment 付款过账：借 default_ap / 贷 default_bank
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn k2_purchase_payment_confirm_posts_gl() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let cj = app.state.cash_journal_service();
    let entry = app.state.gl_entry_service();
    let map = app.state.gl_mapping_service();

    let ap_acct = map
        .resolve(&ctx, &mut conn, "default_ap", None)
        .await
        .expect("resolve default_ap");
    let bank_acct = map
        .resolve(&ctx, &mut conn, "default_bank", None)
        .await
        .expect("resolve default_bank");

    let ap_before = entry
        .get_account_balance(&ctx, &mut conn, ap_acct, None, None)
        .await
        .unwrap();
    let bank_before = entry
        .get_account_balance(&ctx, &mut conn, bank_acct, None, None)
        .await
        .unwrap();

    let today = chrono::Utc::now().date_naive();
    let period = format!("{}", today.format("%Y-%m"));
    let amount = Decimal::from(800);
    let source_id = unique_source_id("k2gl");

    // 建 PurchasePayment CJ（借应付 800 / 贷银行 800，明细平衡）
    let id = cj
        .create(
            &ctx,
            &mut conn,
            CreateCashJournalReq {
                journal_type: JournalType::PurchasePayment,
                direction: CashDirection::Outflow,
                amount,
                counterparty: CounterpartyRef::Supplier(supplier_id()),
                source_type: DocumentType::PurchaseOrder,
                source_id,
                bank_account: "TEST".into(),
                transaction_date: today,
                period,
                remark: "e2e k2 付款过账".into(),
                lines: vec![
                    CashJournalLineInput {
                        account_code: "应付账款".into(),
                        debit_amount: amount,
                        credit_amount: Decimal::ZERO,
                        cost_center: None,
                        profit_center: None,
                        remark: "借应付".into(),
                    },
                    CashJournalLineInput {
                        account_code: "银行存款".into(),
                        debit_amount: Decimal::ZERO,
                        credit_amount: amount,
                        cost_center: None,
                        profit_center: None,
                        remark: "贷银行".into(),
                    },
                ],
            },
        )
        .await
        .expect("create cash journal");

    cj.confirm(&ctx, &mut conn, id, None)
        .await
        .expect("confirm posts GL");

    // 应付（贷方科目）：debit+800 → 余额 -800（付款冲销应付）
    let ap_after = entry
        .get_account_balance(&ctx, &mut conn, ap_acct, None, None)
        .await
        .unwrap();
    assert_eq!(
        ap_before - ap_after,
        amount,
        "付款 confirm 后应付余额应减少 {amount}（借方冲销）"
    );

    // 银行（借方科目）：credit+800 → 余额 -800
    let bank_after = entry
        .get_account_balance(&ctx, &mut conn, bank_acct, None, None)
        .await
        .unwrap();
    assert_eq!(
        bank_before - bank_after,
        amount,
        "付款 confirm 后银行余额应减少 {amount}"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  k3 报销付款过账：借 default_expense / 贷 default_bank
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn k3_expense_payment_posts_gl() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let expense_svc = app.state.expense_service();
    let entry = app.state.gl_entry_service();
    let map = app.state.gl_mapping_service();

    let expense_acct = map
        .resolve(&ctx, &mut conn, "default_expense", None)
        .await
        .expect("resolve default_expense");
    let bank_acct = map
        .resolve(&ctx, &mut conn, "default_bank", None)
        .await
        .expect("resolve default_bank");

    let expense_before = entry
        .get_account_balance(&ctx, &mut conn, expense_acct, None, None)
        .await
        .unwrap();
    let bank_before = entry
        .get_account_balance(&ctx, &mut conn, bank_acct, None, None)
        .await
        .unwrap();

    let today = chrono::Utc::now().date_naive();
    let amount = Decimal::from(150);

    // 建报销单 → submit → approve → generate_payment_journal（过账 GL）
    let id = expense_svc
        .create(
            &ctx,
            &mut conn,
            CreateExpenseReq {
                applicant_id: employee_id(),
                department_id: None,
                expense_date: today,
                remark: "e2e k3 报销付款过账".into(),
                sheet_count: 1,
                has_invoice: true,
                attachments: vec![],
                items: vec![ExpenseItemInput {
                    expense_type: ExpenseType::Office,
                    amount,
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

    expense_svc
        .submit(&ctx, &mut conn, id)
        .await
        .expect("submit");
    expense_svc
        .supervisor_approve(&ctx, &mut conn, id,
            abt_core::fms::expense::model::SupervisorApproveReq { remark: None },
        )
        .await
        .expect("supervisor_approve");
    expense_svc
        .finance_approve(&ctx, &mut conn, id,
            abt_core::fms::expense::model::FinanceApproveReq { remark: None },
        )
        .await
        .expect("finance_approve");
    expense_svc
        .approve(&ctx, &mut conn, id)
        .await
        .expect("approve");

    expense_svc
        .pay(&ctx, &mut conn, id, abt_core::fms::expense::model::PayReq {
            payment_bank: "工商银行".into(),
            payment_remark: "e2e gl test".into(),
            payment_date: chrono::Utc::now().date_naive(),
        })
        .await
        .expect("pay posts GL");

    // 费用（借方科目）：debit+150 → 余额 +150
    let expense_after = entry
        .get_account_balance(&ctx, &mut conn, expense_acct, None, None)
        .await
        .unwrap();
    assert_eq!(
        expense_after - expense_before,
        amount,
        "报销付款后费用科目余额应增加 {amount}"
    );

    // 银行（借方科目）：credit+150 → 余额 -150
    let bank_after = entry
        .get_account_balance(&ctx, &mut conn, bank_acct, None, None)
        .await
        .unwrap();
    assert_eq!(
        bank_before - bank_after,
        amount,
        "报销付款后银行余额应减少 {amount}"
    );
}
