use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait WriteOffService: Send + Sync {
    /// Create a write-off record linking a cash journal to a source document.
    /// Idempotency is handled via DB unique index on idempotency_key.
    async fn write_off(
        &self,
        ctx: ServiceContext<'_>,
        req: WriteOffReq,
    ) -> Result<i64, DomainError>;

    /// List write-off records for a given source document.
    async fn list_by_source(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<WriteOff>, DomainError>;

    /// Get the unreconciled amount for a given source document.
    /// Returns source_total - SUM(write_off.amount).
    /// The caller must provide the source_total since source documents live in other modules.
    async fn get_unreconciled_amount(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        source_total: Decimal,
    ) -> Result<Decimal, DomainError>;
}
