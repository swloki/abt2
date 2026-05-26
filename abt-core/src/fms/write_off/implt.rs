use std::sync::Arc;

use chrono::Utc;
use sqlx::PgPool;

use crate::fms::cash_journal::repo::CashJournalRepo;
use crate::fms::enums::JournalStatus;
use crate::fms::write_off::model::*;
use crate::fms::write_off::repo::WriteOffRepo;
use crate::fms::write_off::service::WriteOffService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};
use crate::fms::enums::WriteOffType;

pub struct WriteOffServiceImpl {
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    pool: Arc<PgPool>,
}

impl WriteOffServiceImpl {
    pub fn new(
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        pool: Arc<PgPool>,
    ) -> Self {
        Self { audit, event_bus, pool }
    }

    /// Derive WriteOffType from the source document type.
    fn derive_write_off_type(source_type: DocumentType) -> WriteOffType {
        match source_type {
            // Sales → SalesReceipt
            DocumentType::ShippingRequest
            | DocumentType::SalesOrder
            | DocumentType::Reconciliation
            | DocumentType::SalesReturn => WriteOffType::SalesReceipt,
            // Purchase → PurchasePayment
            DocumentType::PurchaseQuotation
            | DocumentType::PurchaseOrder
            | DocumentType::PurchaseReturn
            | DocumentType::PaymentRequest
            | DocumentType::Invoice => WriteOffType::PurchasePayment,
            // Other document types default to PurchasePayment
            _ => WriteOffType::PurchasePayment,
        }
    }
}

#[async_trait::async_trait]
impl WriteOffService for WriteOffServiceImpl {
    async fn write_off(
        &self,
        ctx: ServiceContext<'_>,
        req: WriteOffReq,
    ) -> Result<i64, DomainError> {
        // Validate amount > 0
        if req.amount <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("amount must be greater than zero"));
        }

        let write_off_type = Self::derive_write_off_type(req.source_type);
        let today = Utc::now().date_naive();

        // Begin independent transaction to guarantee advisory lock scope
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        // Validate cash_journal exists and is Confirmed
        let journal = CashJournalRepo::get_by_id(&mut tx, req.cash_journal_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))?;

        if journal.status != JournalStatus::Confirmed {
            return Err(DomainError::business_rule("CashJournal must be Confirmed"));
        }

        // Per-journal over-write-off check: total write-offs against this journal must not exceed its amount
        let journal_written = WriteOffRepo::sum_written_off_by_journal(&mut tx, req.cash_journal_id)
            .await
            ?;
        if journal_written + req.amount > journal.amount {
            return Err(DomainError::business_rule("OverWriteOffByJournal"));
        }

        // Anti-over-write-off (P0):
        // Advisory lock serializes concurrent write_offs for the same source,
        // then validate unreconciled >= amount
        let already_written = WriteOffRepo::lock_and_sum_written_off(
            &mut tx,
            req.source_type,
            req.source_id,
        )
        .await
        ?;

        let unreconciled = req.source_total - already_written;
        if unreconciled < req.amount {
            // Release lock before returning error
            WriteOffRepo::release_advisory_lock(&mut tx, req.source_type, req.source_id)
                .await
                .ok();
            return Err(DomainError::business_rule("OverWriteOff"));
        }

        // Insert — DB unique index on idempotency_key handles dedup
        let id = match WriteOffRepo::create(
            &mut tx,
            write_off_type,
            &req,
            today,
            ctx.operator_id,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                if let DomainError::Internal(inner) = &e
                    && inner.downcast_ref::<sqlx::Error>()
                        .map(|db_err| matches!(db_err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")))
                        .unwrap_or(false)
                {
                    WriteOffRepo::release_advisory_lock(&mut tx, req.source_type, req.source_id)
                        .await
                        .ok();
                    return Err(DomainError::duplicate("IdempotencyKey"));
                }
                WriteOffRepo::release_advisory_lock(&mut tx, req.source_type, req.source_id)
                    .await
                    .ok();
                return Err(e);
            }
        };

        // Release advisory lock before commit (lock is no longer needed after insert)
        WriteOffRepo::release_advisory_lock(&mut tx, req.source_type, req.source_id)
            .await
            .ok();

        // Audit log
        {
            let mut tx_ctx = ServiceContext::new(
                &mut *tx as crate::shared::types::PgExecutor<'_>,
                ctx.operator_id,
            );
            self.audit
                .record(
                    tx_ctx.reborrow(),
                    "WriteOff",
                    id,
                    AuditAction::Create,
                    Some(serde_json::json!({
                        "write_off_type": write_off_type.as_str(),
                        "cash_journal_id": req.cash_journal_id,
                        "source_type": req.source_type.as_i16(),
                        "source_id": req.source_id,
                        "amount": req.amount,
                    })),
                    None,
                )
                .await?;

            // Publish domain event
            self.event_bus
                .publish(
                    tx_ctx.reborrow(),
                    EventPublishRequest {
                        event_type: DomainEventType::WriteOffCompleted,
                        aggregate_type: "WriteOff".to_string(),
                        aggregate_id: id,
                        payload: serde_json::json!({
                            "write_off_id": id,
                            "write_off_type": write_off_type.as_str(),
                            "cash_journal_id": req.cash_journal_id,
                            "source_type": req.source_type.as_i16(),
                            "source_id": req.source_id,
                            "amount": req.amount,
                        }),
                        idempotency_key: req.idempotency_key,
                    },
                )
                .await?;
        }

        tx.commit().await.map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        Ok(id)
    }

    async fn list_by_source(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<WriteOff>, DomainError> {
        let (items, total) =
            WriteOffRepo::list_by_source(ctx.executor, source_type, source_id, &page)
                .await
                ?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn get_unreconciled_amount(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        source_total: rust_decimal::Decimal,
    ) -> Result<rust_decimal::Decimal, DomainError> {
        let total_written_off =
            WriteOffRepo::sum_written_off_by_source(ctx.executor, source_type, source_id)
                .await
                ?;

        Ok(source_total - total_written_off)
    }
}
