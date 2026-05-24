use async_trait::async_trait;

use super::model::{CreatePurchaseReturnRequest, PurchaseReturn};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

#[async_trait]
pub trait PurchaseReturnService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePurchaseReturnRequest,
    ) -> Result<i64, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReturn, DomainError>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
