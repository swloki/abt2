use async_trait::async_trait;

use super::model::{CreatePurchaseReturnRequest, PurchaseReturn, PurchaseReturnItem, PurchaseReturnQuery};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PageParams, PgExecutor, Result};

#[async_trait]
pub trait PurchaseReturnService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePurchaseReturnRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseReturn>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: PurchaseReturnQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseReturn>>;

    async fn list_items(&self, ctx: &ServiceContext, db: PgExecutor<'_>, return_id: i64) -> Result<Vec<PurchaseReturnItem>>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;
}
