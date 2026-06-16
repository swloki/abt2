use async_trait::async_trait;

use super::model::TaxRate;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait TaxRateService: Send + Sync {
    /// 查询所有启用的税率
    async fn list_active(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<TaxRate>>;

    /// 按主键查询
    async fn get_by_id(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<TaxRate>>;

    /// 按编码查询
    async fn get_by_code(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        code: &str,
    ) -> Result<Option<TaxRate>>;
}
