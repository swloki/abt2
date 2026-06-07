use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use super::model::*;

#[async_trait]
pub trait WorkOrderService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateWorkOrderReq) -> Result<i64>;
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<WorkOrder>;
    async fn release(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    async fn close(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    async fn get_product_name(&self, db: PgExecutor<'_>, product_id: i64) -> Result<Option<String>>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: WorkOrderFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WorkOrder>>;
}
