use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use super::model::*;

#[async_trait]
pub trait WorkReportService: Send + Sync {
    async fn find_by_id(&self, ctx: ServiceContext<'_>, id: i64) -> Result<WorkReport, DomainError>;
    async fn list_by_work_order(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<Vec<WorkReport>, DomainError>;
    async fn list_by_batch(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
    ) -> Result<Vec<WorkReport>, DomainError>;
    async fn calculate_wage(
        &self,
        ctx: ServiceContext<'_>,
        worker_id: i64,
        date_range: DateRange,
    ) -> Result<WageSummary, DomainError>;
}
