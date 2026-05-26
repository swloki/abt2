use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait ProductionReceiptService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateReceiptReq,
    ) -> Result<i64>;
    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ProductionReceipt>;
    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;
}
