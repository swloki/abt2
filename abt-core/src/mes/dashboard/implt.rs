use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
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

    async fn get_data_quality_stats(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<DataQualityStats> {
        DashboardRepo::get_data_quality_stats(&mut *db).await
    }

    async fn get_quick_entry_stats(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<QuickEntryStats> {
        DashboardRepo::get_quick_entry_stats(&mut *db).await
    }

    async fn get_recent_ops(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, limit: i64) -> Result<Vec<RecentOp>> {
        DashboardRepo::get_recent_ops(&mut *db, limit).await
    }

    async fn get_schedule_stats(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<ScheduleStats> {
        DashboardRepo::get_schedule_stats(&mut *db).await
    }

    async fn get_schedule_cards(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<Vec<ScheduleCard>> {
        DashboardRepo::get_schedule_cards(&mut *db).await
    }

    async fn get_wo_basic_info(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64) -> Result<WoBasicInfo> {
        DashboardRepo::get_wo_basic_info(&mut *db, work_order_id).await
    }

    async fn get_bom_comparison(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64) -> Result<Vec<BomCompareItem>> {
        DashboardRepo::get_bom_comparison(&mut *db, work_order_id).await
    }

    async fn get_gantt_data(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        from: NaiveDate,
        to: NaiveDate,
        work_center_ids: Option<&[i64]>,
    ) -> Result<GanttData> {
        // 工作中心列表（行头）
        let work_centers =
            DashboardRepo::get_active_work_centers(&mut *db, work_center_ids).await?;
        let wc_ids: Vec<i64> = work_centers.iter().map(|wc| wc.id).collect();
        if wc_ids.is_empty() {
            return Ok(GanttData {
                work_centers: vec![],
                bookings: vec![],
                date_range: generate_date_range(from, to),
            });
        }

        // 日期范围 → DateTime 边界（booking 查询用）
        let from_dt = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let to_dt = to.and_hms_opt(23, 59, 59).unwrap().and_utc();

        let bookings =
            DashboardRepo::get_gantt_bookings(&mut *db, &wc_ids, from_dt, to_dt).await?;

        Ok(GanttData {
            work_centers,
            bookings,
            date_range: generate_date_range(from, to),
        })
    }

    async fn get_work_center_load(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<WcDailyLoad>> {
        // 取所有活跃工作中心
        let wcs =
            DashboardRepo::get_active_work_centers(&mut *db, None).await?;
        let wc_ids: Vec<i64> = wcs.iter().map(|wc| wc.id).collect();
        if wc_ids.is_empty() {
            return Ok(vec![]);
        }
        DashboardRepo::get_work_center_load(&mut *db, &wc_ids, from, to).await
    }
}

/// 生成日期序列 [from, to)
fn generate_date_range(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut cur = from;
    while cur < to {
        dates.push(cur);
        cur += Duration::days(1);
    }
    dates
}
