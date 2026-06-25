use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::{PgExecutor, Result};

use super::model::PurchaseWorkCenterSummary;

/// 采购作业中心聚合服务（只读视图，写操作复用各域既有 Service）。
#[async_trait]
pub trait PurchaseWorkCenterService: Send + Sync {
    /// 聚合各业务分组待办计数（首页锚点条 + 各 card 用）。
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseWorkCenterSummary>;
}
