use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
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

    async fn list_all_wage_summaries(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        date_range: DateRange,
    ) -> Result<Vec<WageSummary>>;
    async fn get_detail_lookups(
        &self,
        db: PgExecutor<'_>,
        report: &WorkReport,
    ) -> Result<ReportDetailLookups>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReportListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReportListItem>>;
}
