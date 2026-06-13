use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait MesDashboardService: Send + Sync {
    async fn get_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<DashboardStats>;
    async fn get_data_quality_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<DataQualityStats>;
    async fn get_quick_entry_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<QuickEntryStats>;
    async fn get_recent_ops(&self, ctx: &ServiceContext, db: PgExecutor<'_>, limit: i64) -> Result<Vec<RecentOp>>;
    async fn get_schedule_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<ScheduleStats>;
    async fn get_schedule_cards(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<Vec<ScheduleCard>>;
    async fn get_wo_basic_info(&self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64) -> Result<WoBasicInfo>;
    async fn get_bom_comparison(&self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64) -> Result<Vec<BomCompareItem>>;
}
