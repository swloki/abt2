use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::wms::enums::{PickType, PutawayType};

use super::model::{PickStrategy, PutawayStrategy};

#[async_trait]
pub trait StrategyService: Send + Sync {
    /// 创建上架策略，返回策略 ID
    async fn create_putaway(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        name: String,
        strategy_type: PutawayType,
        warehouse_id: Option<i64>,
        priority: i32,
    ) -> Result<i64>;

    /// 查询上架策略，warehouse_id 为 None 时返回所有
    async fn list_putaway(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<PutawayStrategy>>;

    /// 创建拣货策略，返回策略 ID
    async fn create_pick(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        name: String,
        strategy_type: PickType,
        warehouse_id: Option<i64>,
        priority: i32,
    ) -> Result<i64>;

    /// 查询拣货策略，warehouse_id 为 None 时返回所有
    async fn list_pick(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<PickStrategy>>;
}
