use async_trait::async_trait;

use super::model::{PendingTask, PendingTaskFilter, WorkCenterDomain, WorkCenterSummary};
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PaginatedResult, PgExecutor, Result, ServiceContext};

/// 仓库作业中心服务 — 跨域聚合待办（执行层工作台，非计划层需求池）。
///
/// - `summary`：各环节 (total/overdue/soon) 统计（锚点条 chip 计数 + tab badge + 染色）
/// - `list_pending`：某 tab 环节的待办队列（**数据库分页** + keyword/紧急度/来源过滤，按紧急度排序）
///
/// 实现委托 `WorkCenterRepo` 跨域聚合视图查询。查询失败容错（不连累整页）。
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 各环节待办统计（total + overdue/soon），首屏锚点条 / tab badge / 染色用
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary>;

    /// 某 tab 环节的待办队列（数据库分页，按紧急度 → 到期日排序）。
    /// `filter`（keyword / 紧急度 / 来源）下推 SQL；`page` 数据库分页。
    async fn list_pending(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        filter: PendingTaskFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>>;
}
