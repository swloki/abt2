use async_trait::async_trait;

use crate::shared::types::{PgExecutor, Result, ServiceContext};

use super::model::*;

#[async_trait]
pub trait ProfitCenterService: Send + Sync {
    /// 列出全部启用的利润中心（量少，不分页；前端映射/下拉用）
    async fn list(&self, db: PgExecutor<'_>) -> Result<Vec<ProfitCenter>>;
    async fn get(&self, db: PgExecutor<'_>, id: i64) -> Result<ProfitCenter>;
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateProfitCenterReq,
    ) -> Result<i64>;
    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateProfitCenterReq,
    ) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
