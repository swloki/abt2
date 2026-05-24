use async_trait::async_trait;

use super::model::{
    CreatePurchaseQuotationRequest, PurchaseQuotation, PurchaseQuotationQuery, QuotationComparison,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait PurchaseQuotationService: Send + Sync {
    async fn create(
        ctx: ServiceContext<'_>,
        req: CreatePurchaseQuotationRequest,
    ) -> Result<i64, DomainError>;

    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseQuotation, DomainError>;

    async fn activate(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn list(
        ctx: ServiceContext<'_>,
        query: PurchaseQuotationQuery,
    ) -> Result<PaginatedResult<PurchaseQuotation>, DomainError>;

    async fn compare(
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>, DomainError>;
}
