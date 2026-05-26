use chrono::{DateTime, Utc};
use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub struct PriceRepo;

impl PriceRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        product_id: i64,
        price_type: PriceType,
        old_price: Option<Decimal>,
        new_price: Decimal,
        operator_id: i64,
        remark: &str,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO price_log (product_id, price_type, old_price, new_price, operator_id, remark) VALUES ($1, $2, $3, $4, $5, $6) RETURNING log_id",
        )
        .bind(product_id)
        .bind(price_type.as_i16())
        .bind(old_price)
        .bind(new_price)
        .bind(operator_id)
        .bind(remark)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn find_latest_price(
        &self,
        executor: PgExecutor<'_>,
        product_id: i64,
        price_type: PriceType,
    ) -> Result<Option<PriceLogEntry>> {
        let entry = sqlx::query_as::<sqlx::Postgres, PriceLogEntry>(
            "SELECT log_id, product_id, price_type, old_price, new_price, operator_id, remark, created_at FROM price_log WHERE product_id = $1 AND price_type = $2 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(product_id)
        .bind(price_type.as_i16())
        .fetch_optional(executor)
        .await?;
        Ok(entry)
    }

    pub async fn find_price_at(
        &self,
        executor: PgExecutor<'_>,
        product_id: i64,
        price_type: PriceType,
        as_of: DateTime<Utc>,
    ) -> Result<Option<PriceLogEntry>> {
        let entry = sqlx::query_as::<sqlx::Postgres, PriceLogEntry>(
            "SELECT log_id, product_id, price_type, old_price, new_price, operator_id, remark, created_at FROM price_log WHERE product_id = $1 AND price_type = $2 AND created_at <= $3 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(product_id)
        .bind(price_type.as_i16())
        .bind(as_of)
        .fetch_optional(executor)
        .await?;
        Ok(entry)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &PriceQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<PriceLogEntry>> {
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        let pid_param = if let Some(pid) = filter.product_id {
            param_idx += 1;
            conditions.push(format!("product_id = ${param_idx}"));
            Some(pid)
        } else { None };

        let pt_param = if let Some(pt) = filter.price_type {
            param_idx += 1;
            conditions.push(format!("price_type = ${param_idx}"));
            Some(pt.as_i16())
        } else { None };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM price_log WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(v) = pid_param { count_q = count_q.bind(v); }
        if let Some(v) = pt_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT log_id, product_id, price_type, old_price, new_price, operator_id, remark, created_at FROM price_log WHERE {where_clause} ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, PriceLogEntry>(&data_sql);
        if let Some(v) = pid_param { data_q = data_q.bind(v); }
        if let Some(v) = pt_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// Upsert price — only inserts when price changed (Excel import support)
    pub async fn upsert_price(
        executor: PgExecutor<'_>,
        product_id: i64,
        new_price: Decimal,
        remark: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO price_log (product_id, price_type, new_price, remark)
               SELECT $1, 1, $2, $3
               WHERE COALESCE(
                 (SELECT new_price FROM price_log WHERE product_id = $1 AND price_type = 1 ORDER BY created_at DESC LIMIT 1),
                 -999999999
               ) != $2"#,
        )
        .bind(product_id)
        .bind(new_price)
        .bind(remark)
        .execute(executor)
        .await?;
        Ok(())
    }
}
