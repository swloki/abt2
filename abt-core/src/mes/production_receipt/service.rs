use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use super::model::*;

#[async_trait]
pub trait ProductionReceiptService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateReceiptReq,
    ) -> Result<i64, DomainError>;
    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ProductionReceipt, DomainError>;
    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
