use std::sync::Arc;

use crate::fms::cash_journal::model::*;
use crate::fms::cash_journal::repo::{CashJournalLineRepo, CashJournalRepo};
use crate::fms::cash_journal::service::CashJournalService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct CashJournalServiceImpl {
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    idempotency: Arc<dyn IdempotencyService>,
}

impl CashJournalServiceImpl {
    pub fn new(
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        idempotency: Arc<dyn IdempotencyService>,
    ) -> Self {
        Self {
            doc_seq,
            state_machine,
            audit,
            event_bus,
            idempotency,
        }
    }
}

#[async_trait::async_trait]
impl CashJournalService for CashJournalServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateCashJournalReq,
    ) -> Result<i64, DomainError> {
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

        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::CashJournal)
            .await?;

        let id = CashJournalRepo::create(ctx.executor, &doc_number, &req, ctx.operator_id)
            .await
            ?;

        CashJournalLineRepo::batch_insert(ctx.executor, id, &req.lines)
            .await
            ?;

        self.state_machine
            .transition(ctx.reborrow(), "JournalStatus", id, "Draft", None)
            .await?;

        self.audit
            .record(
                ctx.reborrow(),
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
        mut ctx: ServiceContext<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<(), DomainError> {
        // Step 1: Idempotency check
        if let Some(ref key) = idempotency_key {
            let hash = crate::shared::idempotency::service::key_to_i64(key);
            let is_first = self
                .idempotency
                .check_and_mark(ctx.reborrow(), hash, "CashJournal:confirm")
                .await?;
            if !is_first {
                return Ok(());
            }
        }

        // Step 2: Lock journal FOR UPDATE
        let journal = CashJournalRepo::get_for_update(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))?;

        if journal.status != super::super::enums::JournalStatus::Draft {
            return Err(DomainError::business_rule(
                "Only Draft journals can be confirmed",
            ));
        }

        // Step 3: Aggregate lines debit/credit
        let (total_debit, total_credit) = CashJournalLineRepo::sum_debit_credit(ctx.executor, id)
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
        self.state_machine
            .transition(ctx.reborrow(), "JournalStatus", id, "Confirmed", None)
            .await?;

        // Update status with optimistic lock
        let rows = CashJournalRepo::update_status(
            ctx.executor,
            id,
            super::super::enums::JournalStatus::Confirmed,
            journal.version,
        )
        .await
        ?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        self.audit
            .record(
                ctx.reborrow(),
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

        self.event_bus
            .publish(
                ctx.reborrow(),
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

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<CashJournal, DomainError> {
        CashJournalRepo::get_by_id(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: CashJournalFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<CashJournal>, DomainError> {
        let (items, total) =
            CashJournalRepo::query(
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

    async fn get_balance(
        &self,
        ctx: ServiceContext<'_>,
        period: String,
    ) -> Result<BalanceSummary, DomainError> {
        let (total_inflow, total_outflow) =
            CashJournalRepo::sum_balance_by_period(ctx.executor, &period)
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
