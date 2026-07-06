use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};

use super::model::FmsWorkCenterSummary;

/// 财务作业中心聚合服务（只读视图，写操作复用 fms 各域既有 Service）。
#[async_trait]
pub trait FmsWorkCenterService: Send + Sync {
    /// 聚合各 tab 待办计数 + 顶栏 pill（逾期/临期金额）。首页锚点 + tab badge + pill 用。
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<FmsWorkCenterSummary>;
}
