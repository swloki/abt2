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

    /// Get the total written-off amount for a given source document.
    /// The caller compares this with the source total to compute unreconciled amount.
    async fn get_unreconciled_amount(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<Decimal, DomainError>;
}
