use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use super::model::*;

#[async_trait]
pub trait ProductionPlanService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreatePlanReq) -> Result<i64, DomainError>;
    async fn find_by_id(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ProductionPlan, DomainError>;
    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn release_to_work_orders(
        &self,
        ctx: ServiceContext<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult, DomainError>;
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: PlanFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ProductionPlan>, DomainError>;
}
