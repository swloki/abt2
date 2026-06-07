use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait ProductionExceptionService: Send + Sync {
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ProductionException>;
    async fn list(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ExceptionListFilter, page: u32, page_size: u32,
    ) -> Result<crate::shared::types::PaginatedResult<ExceptionListItem>>;
    async fn get_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<ExceptionStats>;
    async fn update_status(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, status: super::super::enums::ExceptionStatus) -> Result<()>;
    async fn list_events(&self, ctx: &ServiceContext, db: PgExecutor<'_>, exception_id: i64) -> Result<Vec<ExceptionEvent>>;
    async fn get_detail_lookups(&self, db: PgExecutor<'_>, exc: &ProductionException) -> Result<ExceptionDetailLookups>;
}
