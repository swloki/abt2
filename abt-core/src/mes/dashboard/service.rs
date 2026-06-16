use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use chrono::NaiveDate;
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
    /// 甘特图数据：工作中心列表 + 时间范围内的 booking（含工单/产品/工序信息）
    async fn get_gantt_data(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        from: NaiveDate,
        to: NaiveDate,
        work_center_ids: Option<&[i64]>,
    ) -> Result<GanttData>;
    /// 负荷分析：工作中心 × 日期 的已排程工时 vs 可用工时
    async fn get_work_center_load(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<WcDailyLoad>>;
}
