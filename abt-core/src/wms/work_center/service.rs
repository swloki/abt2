use async_trait::async_trait;

use super::model::{PendingTask, UrgentSummary, WorkCenterDomain, WorkCenterSummary};
use crate::shared::types::pagination::PageParams;
use crate::shared::types::{PaginatedResult, PgExecutor, Result, ServiceContext};

/// 仓库作业中心服务 — 聚合各域待办（执行层工作台，非计划层需求池）。
///
/// - `summary`：各环节待办计数（首屏锚点条 chip 计数）
/// - `list_pending`：某环节的待办队列（disclosure 展开懒加载，按紧急度排序）
/// - `urgent_summary`：逾期 / 临期汇总（摘要带染色；消化 #93 followup P1 item 4）
///
/// 全程经各域 Service trait，禁止跨域 repo 直访。查询失败容错（不连累整页）。
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 聚合各环节待办计数（作业中心首页用）
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<WorkCenterSummary>;

    /// 某环节的待办单据队列（disclosure 展开懒加载，按紧急度 → 到期日排序）
    async fn list_pending(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        domain: WorkCenterDomain,
        page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>>;

    /// 紧急 / 临期汇总（锚点条 + 摘要带染色；消化 #93 followup P1 item 4）
    async fn urgent_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<UrgentSummary>;
}
