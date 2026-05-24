use std::sync::Arc;

use chrono::Utc;

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
}

impl WriteOffServiceImpl {
    pub fn new(
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
    ) -> Self {
        Self { audit, event_bus }
    }

    /// Derive WriteOffType from the source document type.
    fn derive_write_off_type(source_type: DocumentType) -> WriteOffType {
        match source_type {
            DocumentType::ShippingRequest
            | DocumentType::SalesOrder
            | DocumentType::Reconciliation
            | DocumentType::SalesReturn => WriteOffType::SalesReceipt,
            _ => WriteOffType::PurchasePayment,
        }
    }
}

#[async_trait::async_trait]
impl WriteOffService for WriteOffServiceImpl {
    async fn write_off(
        &self,
        mut ctx: ServiceContext<'_>,
        req: WriteOffReq,
    ) -> Result<i64, DomainError> {
        // Validate amount > 0
        if req.amount <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("amount must be greater than zero"));
        }

        let write_off_type = Self::derive_write_off_type(req.source_type);
        let today = Utc::now().date_naive();

        // Insert — DB unique index on idempotency_key handles dedup
        let id = match WriteOffRepo::create(
            ctx.executor,
            write_off_type,
            &req,
            today,
            ctx.operator_id,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                // Check for unique constraint violation on idempotency_key
                if let Some(sqlx::Error::Database(db_err)) = e.downcast_ref::<sqlx::Error>()
                    && db_err.code().as_deref() == Some("23505")
                {
                    return Err(DomainError::duplicate("WriteOff"));
                }
                return Err(DomainError::Internal(e));
            }
        };

        // Audit log
        self.audit
            .record(
                ctx.reborrow(),
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
                ctx.reborrow(),
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
                .map_err(DomainError::Internal)?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn get_unreconciled_amount(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<rust_decimal::Decimal, DomainError> {
        let total_written_off =
            WriteOffRepo::sum_written_off_by_source(ctx.executor, source_type, source_id)
                .await
                .map_err(DomainError::Internal)?;

        Ok(total_written_off)
    }
}
