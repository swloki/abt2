use sqlx::PgPool;

use crate::fms::cash_journal::model::*;
use crate::fms::cash_journal::repo::{CashJournalLineRepo, CashJournalRepo};
use crate::fms::cash_journal::service::CashJournalService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::audit_log::new_audit_log_service;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::document_sequence::new_document_sequence_service;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::event_bus::new_domain_event_bus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::shared::idempotency::new_idempotency_service;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::state_machine::new_state_machine_service;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct CashJournalServiceImpl {
    pool: PgPool,
}

impl CashJournalServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl CashJournalService for CashJournalServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateCashJournalReq,
    ) -> Result<i64> {
        if req.amount <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("amount must be greater than zero"));
        }
        if req.lines.is_empty() {
            return Err(DomainError::validation("at least one journal line is required"));
        }

        // Header amount must equal sum of line debit (and credit) totals
        let total_debit: rust_decimal::Decimal = req.lines.iter().map(|l| l.debit_amount).sum();
        let total_credit: rust_decimal::Decimal = req.lines.iter().map(|l| l.credit_amount).sum();
        if total_debit != req.amount || total_credit != req.amount {
            return Err(DomainError::validation(
                "header amount must equal line debit and credit totals",
            ));
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::CashJournal)
            .await?;

        let id = CashJournalRepo::create(db, &doc_number, &req, ctx.operator_id)
            .await
            ?;

        CashJournalLineRepo::batch_insert(db, id, &req.lines)
            .await
            ?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "JournalStatus", id, "Draft", None)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                "CashJournal",
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(id)
    }

    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        // Step 1: Idempotency check
        if let Some(ref key) = idempotency_key {
            let hash = crate::shared::idempotency::service::key_to_i64(key);
            let is_first = new_idempotency_service(self.pool.clone())
                .check_and_mark(ctx, db, hash, "CashJournal:confirm")
                .await?;
            if !is_first {
                return Ok(());
            }
        }

        // Step 2: Lock journal FOR UPDATE
        let journal = CashJournalRepo::get_for_update(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))?;

        if journal.status != super::super::enums::JournalStatus::Draft {
            return Err(DomainError::business_rule(
                "Only Draft journals can be confirmed",
            ));
        }

        // Step 3: Aggregate lines debit/credit
        let (total_debit, total_credit) = CashJournalLineRepo::sum_debit_credit(db, id)
            .await
            ?;

        // Step 4: Validate balanced entry with non-zero totals
        if total_debit != total_credit {
            return Err(DomainError::business_rule("UnbalancedEntry"));
        }
        if total_debit == rust_decimal::Decimal::ZERO {
            return Err(DomainError::business_rule("ZeroEntry"));
        }

        // Step 5: State transition
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "JournalStatus", id, "Confirmed", None)
            .await?;

        // Update status with optimistic lock
        let rows = CashJournalRepo::update_status(
            db,
            id,
            super::super::enums::JournalStatus::Confirmed,
            journal.version,
        )
        .await
        ?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                "CashJournal",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({
                    "from": "Draft",
                    "to": "Confirmed",
                })),
                None,
            )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::CashJournalConfirmed,
                    aggregate_type: "CashJournal".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "cash_journal_id": id,
                        "doc_number": journal.doc_number,
                        "amount": journal.amount,
                        "period": journal.period,
                    }),
                    idempotency_key,
                },
            )
            .await?;

        Ok(())
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<CashJournal> {
        CashJournalRepo::get_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: CashJournalFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<CashJournal>> {
        let (items, total) =
            CashJournalRepo::query(
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

    async fn get_balance(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        period: String,
    ) -> Result<BalanceSummary> {
        let (total_inflow, total_outflow) =
            CashJournalRepo::sum_balance_by_period(db, &period)
                .await
                ?;

        let net_balance = total_inflow - total_outflow;

        Ok(BalanceSummary {
            total_inflow,
            total_outflow,
            net_balance,
            currency: "CNY".to_string(),
        })
    }
}
