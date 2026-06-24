use async_trait::async_trait;

use super::model::WorkCenterSummary;
use crate::shared::types::{PgExecutor, Result, ServiceContext};

/// 仓库作业中心服务 — 聚合各域待办计数（执行层看板，非计划层需求池）。
///
/// 仅做聚合计数；各域列表复用现有 service（前端按状态筛选跳转），不重复实现列表。
/// 全程经各域 Service trait 取 total，禁止跨域 repo 直访。
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 聚合各环节待办计数（作业中心首页用）
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<WorkCenterSummary>;
}
