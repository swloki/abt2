use async_trait::async_trait;

use super::model::{CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderQuery};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait PurchaseOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePurchaseOrderRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64, DomainError>;

    async fn create_from_quotation(
        &self,
        ctx: ServiceContext<'_>,
        quotation_id: i64,
        idempotency_key: Option<String>,
    ) -> Result<i64, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseOrder, DomainError>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: PurchaseOrderQuery,
    ) -> Result<PaginatedResult<PurchaseOrder>, DomainError>;
}
