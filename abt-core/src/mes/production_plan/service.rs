use std::collections::HashMap;

use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use super::model::*;

#[async_trait]
pub trait ProductionPlanService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreatePlanReq) -> Result<i64>;
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ProductionPlan>;
    async fn list_items(&self, ctx: &ServiceContext, db: PgExecutor<'_>, plan_id: i64) -> Result<Vec<ProductionPlanItem>>;
    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn release_to_work_orders(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: PlanFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ProductionPlan>>;
    async fn get_plan_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>, plan_ids: &[i64]) -> Result<HashMap<i64, PlanExtraStats>>;
}
