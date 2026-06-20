use chrono::Datelike;
use sqlx::PgPool;
use rust_decimal::Decimal;

use crate::fms::cash_journal::repo::{CashJournalLineRepo, CashJournalRepo};
use crate::fms::enums::{
    CashDirection, ExpenseStatus, JournalStatus, JournalType,
};
use crate::fms::expense::model::*;
use crate::fms::expense::repo::{ExpenseReimbursementItemRepo, ExpenseReimbursementRepo};
use crate::gl::entry::{model::GlEntryLineInput, new_gl_entry_service, service::GlEntryService};
use crate::gl::mapping::{new_gl_mapping_service, service::GlMappingService};
use crate::fms::expense::service::ExpenseReimbursementService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::document_sequence::new_document_sequence_service;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::event_bus::new_domain_event_bus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::state_machine::new_state_machine_service;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, Result};

pub struct ExpenseReimbursementServiceImpl {
    pool: PgPool,
}

impl ExpenseReimbursementServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 报销付款过账：借 default_expense / 贷 default_bank，金额 = expense.total_amount。
    /// 辅助核算取第一条报销明细的 cost_center/profit_center。
    async fn post_expense_gl(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense: &ExpenseReimbursement,
    ) -> Result<()> {
        let map = new_gl_mapping_service(self.pool.clone());
        let expense_acct = map.resolve(ctx, db, "default_expense", None).await?;
        let bank_acct = map.resolve(ctx, db, "default_bank", None).await?;

        let items = ExpenseReimbursementItemRepo::get_by_reimbursement_id(db, expense.id).await?;
        let (cc, pc) = items
            .first()
            .map(|i| (i.cost_center, i.profit_center))
            .unwrap_or((None, None));

        let amt = expense.total_amount;
        let gl_lines = vec![
            GlEntryLineInput {
                account_id: expense_acct,
                debit: amt,
                credit: Decimal::ZERO,
                cost_center: cc,
                profit_center: pc,
                project_id: None,
                memo: format!("报销付款 {}", expense.doc_number),
            },
            GlEntryLineInput {
                account_id: bank_acct,
                debit: Decimal::ZERO,
                credit: amt,
                cost_center: cc,
                profit_center: pc,
                project_id: None,
                memo: format!("报销付款 {}", expense.doc_number),
            },
        ];

        new_gl_entry_service(self.pool.clone())
            .post_from_source(
                ctx,
                db,
                DocumentType::ExpenseReimbursement,
                expense.id,
                expense.expense_date,
                format!("报销付款 {}", expense.doc_number),
                gl_lines,
            )
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ExpenseReimbursementService for ExpenseReimbursementServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
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
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ExpenseReimbursement)
            .await?;

        // Step 3: Insert expense reimbursement
        let id = ExpenseReimbursementRepo::create(
            db,
            &doc_number,
            &req,
            total_amount,
            ctx.operator_id,
        )
        .await
        ?;

        // Step 4: Batch insert items
        ExpenseReimbursementItemRepo::batch_insert(db, id, &req.items)
            .await
            ?;

        // Step 5: State machine transition to Draft
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Draft", None)
            .await?;

        // Step 6: Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "ExpenseReimbursement", entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        Ok(id)
    }

    async fn submit(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Draft {
            return Err(DomainError::business_rule("Only Draft expenses can be submitted"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Submitted", None)
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(
            db,
            id,
            ExpenseStatus::Submitted,
            expense.version,
        )
        .await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "Draft", "to": "Submitted" })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Submitted {
            return Err(DomainError::business_rule("Only Submitted expenses can be approved"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Approved", None)
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(
            db,
            id,
            ExpenseStatus::Approved,
            expense.version,
        )
        .await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "Submitted", "to": "Approved" })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ExpenseReimbursement> {
        ExpenseReimbursementRepo::get_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>> {
        let (items, total) =
            ExpenseReimbursementRepo::query(
                db,
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

    async fn list_items(
        &self, _ctx: &ServiceContext, db: PgExecutor<'_>,
        reimbursement_id: i64,
    ) -> Result<Vec<ExpenseReimbursementItem>> {
        ExpenseReimbursementItemRepo::get_by_reimbursement_id(db, reimbursement_id).await
    }

    /// IndependentTx — opens its own transaction from PgPool.
    /// Called by WorkflowEngine Hook with ServiceContext for interface alignment.
    async fn generate_payment_journal(
        &self,
        ctx: &ServiceContext, _db: PgExecutor<'_>,
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
            let cj_ctx = ServiceContext::new(expense.operator_id);
            new_document_sequence_service(self.pool.clone())
                .next_number(&cj_ctx, &mut tx, DocumentType::CashJournal)
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

        // 业财一体：报销付款同事务过账到 GL（硬错误，? 传播 → 过账失败整 generate_payment_journal 回滚）
        // 借 default_expense / 贷 default_bank，source_type=ExpenseReimbursement
        self.post_expense_gl(ctx, &mut *tx, &expense).await?;

        // Step 7: Commit transaction
        tx.commit().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Step 8: Publish event — failure does not affect committed business data
        if let Ok(mut event_conn) = self.pool.acquire().await {
            let event_ctx = ServiceContext::new(expense.operator_id);
            new_domain_event_bus(self.pool.clone())
                .publish(
                    &event_ctx, &mut event_conn,
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

    async fn list_pending(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        limit: i64,
    ) -> Result<Vec<ExpenseReimbursement>> {
        let filter = ExpenseFilter { status: vec![ExpenseStatus::Submitted], ..Default::default() };
        let page = PageParams::new(1, limit as u32);
        let (items, _) = ExpenseReimbursementRepo::query(
            db, &filter, &page, ctx.data_scope, ctx.operator_id, ctx.department_id,
        ).await?;
        Ok(items)
    }

    async fn pending_summary(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<(i64, Decimal)> {
        ExpenseReimbursementRepo::pending_summary(db).await
    }
}
