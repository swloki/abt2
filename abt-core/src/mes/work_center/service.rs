use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};

use super::model::MesWorkCenterSummary;

/// MES 生产作业中心聚合服务（只读视图，写操作复用各域既有 Service）。
#[async_trait]
pub trait MesWorkCenterService: Send + Sync {
    /// 聚合各状态工单计数（首页锚点条用）。
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<MesWorkCenterSummary>;
}
