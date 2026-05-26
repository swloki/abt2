use std::sync::Arc;

use chrono::Datelike;
use sqlx::PgPool;

use crate::fms::cash_journal::repo::{CashJournalLineRepo, CashJournalRepo};
use crate::fms::enums::{
    CashDirection, ExpenseStatus, JournalStatus, JournalType,
};
use crate::fms::expense::model::*;
use crate::fms::expense::repo::{ExpenseReimbursementItemRepo, ExpenseReimbursementRepo};
use crate::fms::expense::service::ExpenseReimbursementService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PageParams, PaginatedResult};

pub struct ExpenseReimbursementServiceImpl {
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    pool: Arc<PgPool>,
}

impl ExpenseReimbursementServiceImpl {
    pub fn new(
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        pool: Arc<PgPool>,
    ) -> Self {
        Self {
            doc_seq,
            state_machine,
            audit,
            event_bus,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl ExpenseReimbursementService for ExpenseReimbursementServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateExpenseReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::validation("at least one expense item is required"));
        }

        // Validate each item amount is positive
        for item in &req.items {
            if item.amount <= rust_decimal::Decimal::ZERO {
                return Err(DomainError::validation(
                    "expense item amount must be greater than zero",
                ));
            }
        }

        // Step 1: Calculate total_amount from items
        let total_amount: rust_decimal::Decimal =
            req.items.iter().map(|i| i.amount).sum();

        if total_amount <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("total amount must be greater than zero"));
        }

        // Step 2: Generate doc number
        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::ExpenseReimbursement)
            .await?;

        // Step 3: Insert expense reimbursement
        let id = ExpenseReimbursementRepo::create(
            ctx.executor,
            &doc_number,
            &req,
            total_amount,
            ctx.operator_id,
        )
        .await
        ?;

        // Step 4: Batch insert items
        ExpenseReimbursementItemRepo::batch_insert(ctx.executor, id, &req.items)
            .await
            ?;

        // Step 5: State machine transition to Draft
        self.state_machine
            .transition(ctx.reborrow(), "ExpenseStatus", id, "Draft", None)
            .await?;

        // Step 6: Audit log
        self.audit
            .record(
                ctx.reborrow(),
                "ExpenseReimbursement",
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(id)
    }

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ExpenseReimbursement> {
        ExpenseReimbursementRepo::get_by_id(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>> {
        let (items, total) =
            ExpenseReimbursementRepo::query(
                ctx.executor,
                &filter,
                &page,
                ctx.data_scope,
                ctx.operator_id,
                ctx.department_id,
            )
            .await
            ?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// IndependentTx — opens its own transaction from PgPool.
    /// Called by WorkflowEngine Hook with ServiceContext for interface alignment.
    async fn generate_payment_journal(
        &self,
        _ctx: ServiceContext<'_>,
        expense_id: i64,
    ) -> Result<i64> {
        // Step 1: Begin independent transaction
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 2: Fetch expense (must be Approved)
        let expense = ExpenseReimbursementRepo::get_by_id(&mut tx, expense_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Approved {
            return Err(DomainError::business_rule("InvalidState"));
        }

        // Step 3: Generate a proper CJ doc_number via DocumentSequenceService
        let cj_doc_number = {
            let mut cj_ctx = ServiceContext::new(
                &mut *tx as crate::shared::types::PgExecutor<'_>,
                expense.operator_id,
            );
            self.doc_seq
                .next_number(cj_ctx.reborrow(), DocumentType::CashJournal)
                .await?
        };

        // Step 4: Build CashJournal request and create via Repo
        let now = chrono::Utc::now();
        let journal_req = crate::fms::cash_journal::model::CreateCashJournalReq {
            journal_type: JournalType::Expense,
            direction: CashDirection::Outflow,
            amount: expense.total_amount,
            counterparty: crate::fms::enums::CounterpartyRef::Employee(expense.applicant_id),
            source_type: DocumentType::ExpenseReimbursement,
            source_id: expense.id,
            bank_account: String::new(),
            transaction_date: now.date_naive(),
            period: format!("{}-{:02}", now.year(), now.month()),
            remark: expense.remark.clone(),
            lines: vec![
                crate::fms::cash_journal::model::CashJournalLineInput {
                    account_code: "应付职工薪酬".to_string(),
                    debit_amount: expense.total_amount,
                    credit_amount: rust_decimal::Decimal::ZERO,
                    cost_center: None,
                    profit_center: None,
                    remark: "报销付款 — 借方".to_string(),
                },
                crate::fms::cash_journal::model::CashJournalLineInput {
                    account_code: "银行存款".to_string(),
                    debit_amount: rust_decimal::Decimal::ZERO,
                    credit_amount: expense.total_amount,
                    cost_center: None,
                    profit_center: None,
                    remark: "报销付款 — 贷方".to_string(),
                },
            ],
        };

        // Validate balanced entry (debit == credit, non-zero) before inserting
        let total_debit: rust_decimal::Decimal =
            journal_req.lines.iter().map(|l| l.debit_amount).sum();
        let total_credit: rust_decimal::Decimal =
            journal_req.lines.iter().map(|l| l.credit_amount).sum();
        if total_debit != total_credit {
            return Err(DomainError::business_rule("UnbalancedEntry"));
        }
        if total_debit == rust_decimal::Decimal::ZERO {
            return Err(DomainError::business_rule("ZeroEntry"));
        }

        let journal_id = CashJournalRepo::create(
            &mut tx,
            &cj_doc_number,
            &journal_req,
            expense.operator_id,
        )
        .await
        ?;

        CashJournalLineRepo::batch_insert(
            &mut tx,
            journal_id,
            &journal_req.lines,
        )
        .await
        ?;

        // Step 5: Set journal status directly to Confirmed — check rows affected
        let journal_rows = CashJournalRepo::update_status(
            &mut tx,
            journal_id,
            JournalStatus::Confirmed,
            1, // initial version after create
        )
        .await
        ?;

        if journal_rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Step 6: Update expense status to Paid with optimistic lock
        let rows = ExpenseReimbursementRepo::update_status(
            &mut tx,
            expense.id,
            ExpenseStatus::Paid,
            expense.version,
        )
        .await
        ?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Step 7: Commit transaction
        tx.commit().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 8: Publish event — failure does not affect committed business data
        if let Ok(mut event_conn) = self.pool.acquire().await {
            let event_ctx = ServiceContext::new(
                &mut *event_conn as crate::shared::types::PgExecutor<'_>,
                expense.operator_id,
            );
            self.event_bus
                .publish(
                    event_ctx,
                    EventPublishRequest {
                        event_type: DomainEventType::ExpensePaymentGenerated,
                        aggregate_type: "ExpenseReimbursement".to_string(),
                        aggregate_id: expense.id,
                        payload: serde_json::json!({
                            "expense_id": expense.id,
                            "doc_number": expense.doc_number,
                            "total_amount": expense.total_amount,
                            "cash_journal_id": journal_id,
                            "applicant_id": expense.applicant_id,
                        }),
                        idempotency_key: Some(format!("ExpensePayment:{}", expense.id)),
                    },
                )
                .await
                .ok();
        }

        Ok(journal_id)
    }
}
