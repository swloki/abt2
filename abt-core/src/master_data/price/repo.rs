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
        params: &CreatePriceParams<'_>,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO price_log (product_id, price_type, old_price, new_price, operator_id, remark) VALUES ($1, $2, $3, $4, $5, $6) RETURNING log_id",
        )
        .bind(params.product_id)
        .bind(params.price_type.as_i16())
        .bind(params.old_price)
        .bind(params.new_price)
        .bind(params.operator_id)
        .bind(params.remark)
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
            conditions.push(format!("pl.product_id = ${param_idx}"));
            Some(pid)
        } else { None };
        let pt_param = if let Some(pt) = filter.price_type {
            param_idx += 1;
            conditions.push(format!("pl.price_type = ${param_idx}"));
            Some(pt.as_i16())
        } else { None };
        let kw_param = if let Some(ref kw) = filter.keyword {
            if !kw.is_empty() {
                param_idx += 1;
                conditions.push(format!("(p.product_code ILIKE ${param_idx} OR p.pdt_name ILIKE ${param_idx})"));
                Some(format!("%{kw}%"))
            } else { None }
        } else { None };
        let df_param = if let Some(dt) = filter.date_from {
            param_idx += 1;
            conditions.push(format!("pl.created_at >= ${param_idx}"));
            Some(dt)
        } else { None };
        let dt_param = if let Some(dt) = filter.date_to {
            param_idx += 1;
            conditions.push(format!("pl.created_at < ${param_idx}"));
            Some(dt)
        } else { None };
        let needs_join = kw_param.is_some();
        let join_clause = if needs_join {
            "price_log pl JOIN products p ON pl.product_id = p.product_id"
        } else {
            "price_log pl"
        };
        let select_prefix = "pl.log_id, pl.product_id, pl.price_type, pl.old_price, pl.new_price, pl.operator_id, pl.remark, pl.created_at";
        let where_clause = conditions.join(" AND ");
        let count_sql = format!("SELECT COUNT(*) FROM {join_clause} WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = pid_param { count_q = count_q.bind(v); }
        if let Some(v) = pt_param { count_q = count_q.bind(v); }
        if let Some(ref v) = kw_param { count_q = count_q.bind(v); }
        if let Some(ref v) = df_param { count_q = count_q.bind(v); }
        if let Some(ref v) = dt_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {select_prefix} FROM {join_clause} WHERE {where_clause} ORDER BY pl.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, PriceLogEntry>(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = pid_param { data_q = data_q.bind(v); }
        if let Some(v) = pt_param { data_q = data_q.bind(v); }
        if let Some(ref v) = kw_param { data_q = data_q.bind(v); }
        if let Some(ref v) = df_param { data_q = data_q.bind(v); }
        if let Some(ref v) = dt_param { data_q = data_q.bind(v); }
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
