use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait WorkReportService: Send + Sync {
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<WorkReport>;
    async fn list_by_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<WorkReport>>;
    async fn list_by_batch(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<Vec<WorkReport>>;
    async fn calculate_wage(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        worker_id: i64,
        date_range: DateRange,
    ) -> Result<WageSummary>;
}
