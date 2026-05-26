use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::{PickStrategy, PutawayStrategy};
use crate::wms::enums::{PickType, PutawayType};

pub struct StrategyRepo;

impl StrategyRepo {
    /// INSERT 上架策略，返回生成的实体
    pub async fn insert_putaway(
        executor: &mut sqlx::postgres::PgConnection,
        name: &str,
        strategy_type: PutawayType,
        warehouse_id: Option<i64>,
        priority: i32,
    ) -> Result<PutawayStrategy> {
        let row = sqlx::query(
            r#"
            INSERT INTO putaway_strategies (name, strategy_type, warehouse_id, priority, is_active)
            VALUES ($1, $2, $3, $4, TRUE)
            RETURNING id, name, strategy_type, warehouse_id, product_category_id, priority, is_active
            "#,
        )
        .bind(name)
        .bind(strategy_type)
        .bind(warehouse_id)
        .bind(priority)
        .fetch_one(&mut *executor)
        .await?;

        Ok(PutawayStrategy::from_row(&row)?)
    }

    /// 查询上架策略，warehouse_id 为 Some 时按仓库过滤，仅返回 is_active = true
    pub async fn list_putaway(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<PutawayStrategy>> {
        let rows = if let Some(wh) = warehouse_id {
            sqlx::query(
                r#"
                SELECT id, name, strategy_type, warehouse_id, product_category_id, priority, is_active
                FROM putaway_strategies
                WHERE warehouse_id = $1 AND is_active = TRUE
                ORDER BY priority
                "#,
            )
            .bind(wh)
            .fetch_all(&mut *executor)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, name, strategy_type, warehouse_id, product_category_id, priority, is_active
                FROM putaway_strategies
                WHERE is_active = TRUE
                ORDER BY priority
                "#,
            )
            .fetch_all(&mut *executor)
            .await?
        };

        rows.iter()
            .map(|r| PutawayStrategy::from_row(r).map_err(Into::into))
            .collect()
    }

    /// INSERT 拣货策略，返回生成的实体
    pub async fn insert_pick(
        executor: &mut sqlx::postgres::PgConnection,
        name: &str,
        strategy_type: PickType,
        warehouse_id: Option<i64>,
        priority: i32,
    ) -> Result<PickStrategy> {
        let row = sqlx::query(
            r#"
            INSERT INTO pick_strategies (name, strategy_type, warehouse_id, priority, is_active)
            VALUES ($1, $2, $3, $4, TRUE)
            RETURNING id, name, strategy_type, warehouse_id, priority, is_active
            "#,
        )
        .bind(name)
        .bind(strategy_type)
        .bind(warehouse_id)
        .bind(priority)
        .fetch_one(&mut *executor)
        .await?;

        Ok(PickStrategy::from_row(&row)?)
    }

    /// 查询拣货策略，warehouse_id 为 Some 时按仓库过滤
    pub async fn list_pick(
        executor: &mut sqlx::postgres::PgConnection,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<PickStrategy>> {
        let rows = if let Some(wh) = warehouse_id {
            sqlx::query(
                r#"
                SELECT id, name, strategy_type, warehouse_id, priority, is_active
                FROM pick_strategies
                WHERE warehouse_id = $1 AND is_active = TRUE
                ORDER BY priority
                "#,
            )
            .bind(wh)
            .fetch_all(&mut *executor)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, name, strategy_type, warehouse_id, priority, is_active
                FROM pick_strategies
                WHERE is_active = TRUE
                ORDER BY priority
                "#,
            )
            .fetch_all(&mut *executor)
            .await?
        };

        rows.iter().map(|r| PickStrategy::from_row(r).map_err(Into::into)).collect()
    }
}
