use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::{UpdateWmsSettingsReq, WmsSettings};

#[async_trait]
pub trait WmsSettingsService: Send + Sync {
    /// 读取 WMS 全局设置
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WmsSettings>;

    /// 更新 WMS 全局设置
    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpdateWmsSettingsReq,
    ) -> Result<WmsSettings>;
}
