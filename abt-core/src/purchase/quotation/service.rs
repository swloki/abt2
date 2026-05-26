use async_trait::async_trait;

use super::model::{
    CreatePurchaseQuotationRequest, PurchaseQuotation, PurchaseQuotationQuery, QuotationComparison,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait PurchaseQuotationService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePurchaseQuotationRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseQuotation>;

    async fn activate(&self, ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: PurchaseQuotationQuery,
    ) -> Result<PaginatedResult<PurchaseQuotation>>;

    async fn compare(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>>;
}
