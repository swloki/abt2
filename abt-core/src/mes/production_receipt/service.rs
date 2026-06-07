use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PaginatedResult, PgExecutor};
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait ProductionReceiptService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateReceiptReq,
    ) -> Result<i64>;
    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionReceipt>;
    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReceiptListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReceiptListItem>>;
}
