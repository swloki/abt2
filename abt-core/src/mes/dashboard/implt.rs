use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::DashboardRepo;
use super::service::MesDashboardService;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

pub struct MesDashboardServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl MesDashboardServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MesDashboardService for MesDashboardServiceImpl {
    async fn get_stats(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<DashboardStats> {
        DashboardRepo::get_stats(&mut *db).await
    }

    async fn get_quick_entry_stats(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<QuickEntryStats> {
        DashboardRepo::get_quick_entry_stats(&mut *db).await
    }

    async fn get_recent_ops(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, limit: i64) -> Result<Vec<RecentOp>> {
        DashboardRepo::get_recent_ops(&mut *db, limit).await
    }
}
