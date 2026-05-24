use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{PickStrategy, PutawayStrategy};
use super::repo::StrategyRepo;
use super::service::StrategyService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::wms::enums::{PickType, PutawayType};

pub struct StrategyServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl StrategyServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StrategyService for StrategyServiceImpl {
    async fn create_putaway(
        &self,
        ctx: ServiceContext<'_>,
        name: String,
        strategy_type: PutawayType,
        warehouse_id: Option<i64>,
        priority: i32,
    ) -> Result<i64, DomainError> {
        let strategy = StrategyRepo::insert_putaway(
            &mut *ctx.executor,
            &name,
            strategy_type,
            warehouse_id,
            priority,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(strategy.id)
    }

    async fn list_putaway(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<PutawayStrategy>, DomainError> {
        StrategyRepo::list_putaway(&mut *ctx.executor, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn create_pick(
        &self,
        ctx: ServiceContext<'_>,
        name: String,
        strategy_type: PickType,
        warehouse_id: Option<i64>,
        priority: i32,
    ) -> Result<i64, DomainError> {
        let strategy = StrategyRepo::insert_pick(
            &mut *ctx.executor,
            &name,
            strategy_type,
            warehouse_id,
            priority,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(strategy.id)
    }

    async fn list_pick(
        &self,
        ctx: ServiceContext<'_>,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<PickStrategy>, DomainError> {
        StrategyRepo::list_pick(&mut *ctx.executor, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
