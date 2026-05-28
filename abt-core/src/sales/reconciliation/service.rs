use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, Result, ServiceContext};

#[async_trait]
pub trait ReconciliationService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        customer_id: i64,
        period: String,
    ) -> Result<i64>;

    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Reconciliation>;

    async fn send(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn dispute(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn reopen(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn force_settle(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn settle(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        reconciliation_id: i64,
    ) -> Result<Vec<ReconciliationItem>>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReconciliationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Reconciliation>>;
}
