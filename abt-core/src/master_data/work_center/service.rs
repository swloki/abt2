use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::{PgExecutor, Result};

use super::model::*;

#[async_trait]
pub trait WorkCenterService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateWorkCenterReq,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<WorkCenter>;

    async fn get_by_code(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        code: &str,
    ) -> Result<Option<WorkCenter>>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: WorkCenterFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<WorkCenter>>;

    async fn list_active(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<WorkCenter>>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateWorkCenterReq,
    ) -> Result<()>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
