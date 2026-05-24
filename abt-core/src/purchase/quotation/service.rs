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
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePurchaseQuotationRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseQuotation, DomainError>;

    async fn activate(&self, ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: PurchaseQuotationQuery,
    ) -> Result<PaginatedResult<PurchaseQuotation>, DomainError>;

    async fn compare(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>, DomainError>;
}
