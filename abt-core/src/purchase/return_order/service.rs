use async_trait::async_trait;

use super::model::{CreatePurchaseReturnRequest, PurchaseReturn};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

#[async_trait]
pub trait PurchaseReturnService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreatePurchaseReturnRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReturn>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;
}
