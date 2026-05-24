use std::sync::Arc;

use chrono::Datelike;
use sqlx::PgPool;

use crate::fms::enums::{
    CashDirection, CounterpartyType, ExpenseStatus, JournalStatus, JournalType,
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
    ) -> Result<i64, DomainError> {
        // Step 1: Calculate total_amount from items
        let total_amount: rust_decimal::Decimal =
            req.items.iter().map(|i| i.amount).sum();

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
        .map_err(DomainError::Internal)?;

        // Step 4: Batch insert items
        ExpenseReimbursementItemRepo::batch_insert(ctx.executor, id, &req.items)
            .await
            .map_err(DomainError::Internal)?;

        // Step 5: State machine transition to Draft
        self.state_machine
            .transition(ctx.reborrow(), "ExpenseStatus", id, "Draft", None)
            .await
            .ok();

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

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ExpenseReimbursement, DomainError> {
        ExpenseReimbursementRepo::get_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>, DomainError> {
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
            .map_err(DomainError::Internal)?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// IndependentTx — opens its own transaction from PgPool.
    /// Called by WorkflowEngine Hook (no ServiceContext available).
    async fn generate_payment_journal(&self, expense_id: i64) -> Result<i64, DomainError> {
        // Step 1: Begin independent transaction
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 2: Fetch expense (must be Approved)
        let expense = ExpenseReimbursementRepo::get_by_id(&mut *tx, expense_id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Approved {
            return Err(DomainError::business_rule("InvalidState"));
        }

        // Step 3: Create CashJournal directly via SQL (Outflow, Expense type)
        let journal_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO cash_journals
               (doc_number, journal_type, direction, amount,
                counterparty_type, counterparty_id, source_type, source_id,
                bank_account, transaction_date, period, status, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
               RETURNING id"#,
        )
        .bind(&expense.doc_number)
        .bind(JournalType::Expense)
        .bind(CashDirection::Outflow)
        .bind(expense.total_amount)
        .bind(CounterpartyType::Employee)
        .bind(expense.applicant_id)
        .bind(DocumentType::ExpenseReimbursement.as_i16())
        .bind(expense.id)
        .bind("")
        .bind(chrono::Local::now().date_naive())
        .bind(format!(
            "{}-{:02}",
            chrono::Local::now().year(),
            chrono::Local::now().month()
        ))
        .bind(JournalStatus::Confirmed)
        .bind(&expense.remark)
        .bind(expense.operator_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 4: Create CashJournal lines (one debit + one credit)
        sqlx::query(
            r#"INSERT INTO cash_journal_lines
               (journal_id, account_code, debit_amount, credit_amount, cost_center, profit_center, remark)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(journal_id)
        .bind("应付职工薪酬")
        .bind(expense.total_amount)
        .bind(rust_decimal::Decimal::ZERO)
        .bind::<Option<i64>>(None)
        .bind::<Option<i64>>(None)
        .bind("报销付款 — 借方")
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        sqlx::query(
            r#"INSERT INTO cash_journal_lines
               (journal_id, account_code, debit_amount, credit_amount, cost_center, profit_center, remark)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(journal_id)
        .bind("银行存款")
        .bind(rust_decimal::Decimal::ZERO)
        .bind(expense.total_amount)
        .bind::<Option<i64>>(None)
        .bind::<Option<i64>>(None)
        .bind("报销付款 — 贷方")
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 5: Update expense status to Paid with optimistic lock
        let rows = ExpenseReimbursementRepo::update_status(
            &mut *tx,
            expense.id,
            ExpenseStatus::Paid,
            expense.version,
        )
        .await
        .map_err(DomainError::Internal)?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Step 6: Commit transaction
        tx.commit().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 7: Publish event (outside tx) — create a temporary ServiceContext
        // Use a fresh tx from pool for the event bus publish
        let mut publish_tx = self.pool.begin().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;
        let publish_ctx = ServiceContext::new(&mut *publish_tx as common::PgExecutor<'_>, expense.operator_id);

        self.event_bus
            .publish(
                publish_ctx,
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
            .await?;

        publish_tx.commit().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        Ok(journal_id)
    }
}
