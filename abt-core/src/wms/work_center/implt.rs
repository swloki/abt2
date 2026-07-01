use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use sqlx::postgres::PgPool;
use tracing::warn;

use super::model::{
    DomainStats, PendingTask, PendingTaskFilter, WorkCenterDomain, WorkCenterSummary,
};
use super::repo::WorkCenterRepo;
use super::service::WorkCenterService;
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PaginatedResult, PgExecutor, Result, ServiceContext};

pub struct WorkCenterServiceImpl {
    // pool 保留以维持按需工厂约定；聚合查询走传入的 PgExecutor（事务/exec 共享）
    #[allow(dead_code)]
    pool: PgPool,
}

impl WorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkCenterService for WorkCenterServiceImpl {
    async fn summary(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary> {
        let today = Utc::now().date_naive();
        Ok(WorkCenterSummary {
            arrivals: count_safe(&mut *db, WorkCenterDomain::Arrival, today).await,
            outbounds: count_safe(&mut *db, WorkCenterDomain::Outbound, today).await,
            requisitions: count_safe(&mut *db, WorkCenterDomain::Requisition, today).await,
            transfers: count_safe(&mut *db, WorkCenterDomain::Transfer, today).await,
            cycle_counts: count_safe(&mut *db, WorkCenterDomain::CycleCount, today).await,
        })
    }

    async fn list_pending(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        filter: PendingTaskFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>> {
        let today = Utc::now().date_naive();
        WorkCenterRepo::list_domain_page(db, domain, &filter, today, page).await
    }
}

/// 容错 count：查询失败 log warn 后记 0（聚合看板，部分域可用时仍展示其余域）。
async fn count_safe(
    db: PgExecutor<'_>,
    domain: WorkCenterDomain,
    today: NaiveDate,
) -> DomainStats {
    match WorkCenterRepo::count_domain(db, domain, today).await {
        Ok((total, overdue, soon)) => DomainStats { total, overdue, soon },
        Err(e) => {
            warn!(?domain, error = %e, "work_center count_domain failed, recorded as 0");
            DomainStats::default()
        }
    }
}
