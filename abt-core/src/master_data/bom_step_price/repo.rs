use rust_decimal::Decimal;

use crate::shared::types::{PgExecutor, Result};

use super::model::*;

pub struct BomStepPriceRepo;

impl BomStepPriceRepo {
    /// 按 product_code 取全部单价行（step_order 升序）
    pub async fn find_by_product(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Vec<BomStepPrice>> {
        let rows = sqlx::query_as::<sqlx::Postgres, BomStepPrice>(
            r#"SELECT id, product_code, step_order, unit_price, quantity,
                      operator_id, created_at, updated_at
               FROM bom_step_prices
               WHERE product_code = $1
               ORDER BY step_order"#,
        )
        .bind(product_code)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 单行单价查询（工单 load 用）—— 返回 unit_price
    pub async fn find_price(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
    ) -> Result<Option<Decimal>> {
        let price = sqlx::query_scalar::<sqlx::Postgres, Option<Decimal>>(
            r#"SELECT unit_price FROM bom_step_prices
               WHERE product_code = $1 AND step_order = $2"#,
        )
        .bind(product_code)
        .bind(step_order)
        .fetch_optional(&mut *executor)
        .await?;
        // fetch_optional: None=行不存在, Some(None)=行存在但价 NULL, Some(Some(v))=有价
        Ok(price.flatten())
    }

    /// 校验 bom_operations 有对应行（拒「有价无工序」孤儿，review minor）
    pub async fn operation_exists(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
    ) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                 SELECT 1 FROM bom_operations
                 WHERE product_code = $1 AND step_order = $2
               )"#,
        )
        .bind(product_code)
        .bind(step_order)
        .fetch_one(executor)
        .await?;
        Ok(exists)
    }

    /// UPSERT unit_price（quantity 保留原值；新建行默认 1）。
    /// 返回旧价（用于 history；行不存在或旧价 NULL 均为 None）。
    pub async fn upsert(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
        unit_price: Decimal,
        operator_id: i64,
    ) -> Result<Option<Decimal>> {
        // 先取旧价（history 用）
        let old: Option<Option<Decimal>> = sqlx::query_scalar::<sqlx::Postgres, Option<Decimal>>(
            r#"SELECT unit_price FROM bom_step_prices
               WHERE product_code = $1 AND step_order = $2"#,
        )
        .bind(product_code)
        .bind(step_order)
        .fetch_optional(&mut *executor)
        .await?;
        let old_price = old.flatten();

        // upsert：INSERT 新行（quantity 默认 1）/ UPDATE 仅改 unit_price（quantity 保留）
        sqlx::query(
            r#"INSERT INTO bom_step_prices (product_code, step_order, unit_price, operator_id)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (product_code, step_order) DO UPDATE SET
                 unit_price  = EXCLUDED.unit_price,
                 operator_id = EXCLUDED.operator_id,
                 updated_at  = now()"#,
        )
        .bind(product_code)
        .bind(step_order)
        .bind(unit_price)
        .bind(operator_id)
        .execute(executor)
        .await?;

        Ok(old_price)
    }

    /// 追加 history 一行（R-15）
    pub async fn insert_history(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        step_order: i32,
        old_price: Option<Decimal>,
        new_price: Decimal,
        source_type: &str,
        source_wo_id: Option<i64>,
        operator_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO bom_step_price_history
                 (product_code, step_order, old_price, new_price, quantity,
                  source_type, source_wo_id, operator_id, created_at)
               SELECT $1, $2, $3, $4,
                      COALESCE((SELECT quantity FROM bom_step_prices
                                WHERE product_code = $1 AND step_order = $2), 1),
                      $5, $6, $7, now()"#,
        )
        .bind(product_code)
        .bind(step_order)
        .bind(old_price)
        .bind(new_price)
        .bind(source_type)
        .bind(source_wo_id)
        .bind(operator_id)
        .execute(executor)
        .await?;
        Ok(())
    }
}
