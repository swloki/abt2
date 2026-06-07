use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait MesDashboardService: Send + Sync {
    async fn get_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<DashboardStats>;
    async fn get_quick_entry_stats(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<QuickEntryStats>;
    async fn get_recent_ops(&self, ctx: &ServiceContext, db: PgExecutor<'_>, limit: i64) -> Result<Vec<RecentOp>>;
}
