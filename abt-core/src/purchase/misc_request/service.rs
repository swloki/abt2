use async_trait::async_trait;

use super::model::{CreateMiscRequestRequest, MiscRequestItem, MiscRequestQuery, MiscellaneousRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::PageParams;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait MiscellaneousRequestService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateMiscRequestRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<MiscellaneousRequest>;

    async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MiscRequestQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MiscellaneousRequest>>;

    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        request_id: i64,
    ) -> Result<Vec<MiscRequestItem>>;

    /// 批量取多个零星请购的明细（扁平 Vec，调用方按 request_id 分组）。
    async fn list_items_by_request_ids(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        request_ids: &[i64],
    ) -> Result<Vec<MiscRequestItem>>;

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()>;
}
