use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{ReconciliationItem, ReconciliationQuery, ReconciliationStatement};
use crate::repositories::{build_fuzzy_pattern, Executor, PaginationParams};

pub struct ReconciliationInsertParams<'a> {
    pub statement_no: &'a str,
    pub customer_name: &'a str,
    pub period_year: i16,
    pub period_month: i16,
    pub shipping_total: Decimal,
    pub return_total: Decimal,
    pub adjustment_total: Decimal,
    pub net_amount: Decimal,
    pub remark: Option<&'a str>,
    pub operator_id: Option<i64>,
}

pub struct ReconciliationItemRow<'a> {
    pub source_type: &'a str,
    pub source_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_code: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub remark: Option<&'a str>,
}

pub struct ReconciliationRepo;

impl ReconciliationRepo {
    pub async fn insert(executor: Executor<'_>, p: &ReconciliationInsertParams<'_>) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO reconciliation_statements (statement_no, customer_name, period_year, period_month, shipping_total, return_total, adjustment_total, net_amount, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING statement_id
            "#,
            p.statement_no,
            p.customer_name,
            p.period_year as i32,
            p.period_month as i32,
            p.shipping_total,
            p.return_total,
            p.adjustment_total,
            p.net_amount,
            p.remark,
            p.operator_id,
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn soft_delete(executor: Executor<'_>, statement_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE reconciliation_statements SET deleted_at = NOW() WHERE statement_id = $1",
            statement_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_status(executor: Executor<'_>, statement_id: i64, status: i16) -> Result<()> {
        sqlx::query!(
            "UPDATE reconciliation_statements SET status = $1, updated_at = NOW() WHERE statement_id = $2",
            status,
            statement_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_remark(executor: Executor<'_>, statement_id: i64, remark: Option<&str>) -> Result<()> {
        sqlx::query!(
            "UPDATE reconciliation_statements SET remark = $1, updated_at = NOW() WHERE statement_id = $2",
            remark,
            statement_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, statement_id: i64) -> Result<Option<ReconciliationStatement>> {
        let row = sqlx::query_as::<_, ReconciliationStatement>(
            "SELECT statement_id, statement_no, customer_name, period_year, period_month, \
             shipping_total, return_total, adjustment_total, net_amount, status, remark, \
             operator_id, created_at, updated_at, deleted_at \
             FROM reconciliation_statements WHERE statement_id = $1 AND deleted_at IS NULL",
        )
        .bind(statement_id)
        .fetch_optional(pool)
        .await?;

        if let Some(mut s) = row {
            s.items = Self::find_items_by_statement_id(pool, statement_id).await?;
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }

    pub async fn find_status(pool: &PgPool, statement_id: i64) -> Result<Option<i16>> {
        let row: Option<(i16,)> = sqlx::query_as(
            "SELECT status FROM reconciliation_statements WHERE statement_id = $1 AND deleted_at IS NULL",
        )
        .bind(statement_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn query(pool: &PgPool, q: &ReconciliationQuery) -> Result<(Vec<ReconciliationStatement>, i64)> {
        let pagination = PaginationParams::new(
            q.page.unwrap_or(1) as u32,
            q.page_size.unwrap_or(20) as u32,
        );

        let mut where_clauses = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        let keyword_param = if let Some(ref kw) = q.keyword {
            if let Some(pattern) = build_fuzzy_pattern(kw) {
                param_idx += 1;
                where_clauses.push(format!(
                    "(statement_no ILIKE ${} OR customer_name ILIKE ${})",
                    param_idx, param_idx
                ));
                Some(pattern)
            } else {
                None
            }
        } else {
            None
        };

        let status_param = if let Some(s) = q.status {
            param_idx += 1;
            where_clauses.push(format!("status = ${}", param_idx));
            Some(s)
        } else {
            None
        };

        let year_param = if let Some(y) = q.period_year {
            param_idx += 1;
            where_clauses.push(format!("period_year = ${}", param_idx));
            Some(y)
        } else {
            None
        };

        let month_param = if let Some(m) = q.period_month {
            param_idx += 1;
            where_clauses.push(format!("period_month = ${}", param_idx));
            Some(m)
        } else {
            None
        };

        let where_sql = where_clauses.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM reconciliation_statements WHERE {}", where_sql);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(ref p) = keyword_param { count_query = count_query.bind(p); }
        if let Some(s) = status_param { count_query = count_query.bind(s); }
        if let Some(y) = year_param { count_query = count_query.bind(y); }
        if let Some(m) = month_param { count_query = count_query.bind(m); }
        let total = count_query.fetch_one(pool).await?;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT statement_id, statement_no, customer_name, period_year, period_month, \
             shipping_total, return_total, adjustment_total, net_amount, status, remark, \
             operator_id, created_at, updated_at, deleted_at \
             FROM reconciliation_statements WHERE {} ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            where_sql, limit_idx, offset_idx
        );

        let mut data_query = sqlx::query_as::<_, ReconciliationStatement>(&data_sql);
        if let Some(ref p) = keyword_param { data_query = data_query.bind(p); }
        if let Some(s) = status_param { data_query = data_query.bind(s); }
        if let Some(y) = year_param { data_query = data_query.bind(y); }
        if let Some(m) = month_param { data_query = data_query.bind(m); }
        data_query = data_query
            .bind(pagination.page_size as i64)
            .bind(pagination.offset() as i64);

        let items = data_query.fetch_all(pool).await?;
        Ok((items, total))
    }

    pub async fn insert_items(
        executor: Executor<'_>,
        statement_id: i64,
        items: &[ReconciliationItemRow<'_>],
    ) -> Result<()> {
        for row in items {
            sqlx::query!(
                r#"
                INSERT INTO reconciliation_items (statement_id, source_type, source_id, product_id, product_code, product_name, unit, quantity, unit_price, amount, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
                statement_id,
                row.source_type,
                row.source_id,
                row.product_id,
                row.product_code,
                row.product_name,
                row.unit,
                row.quantity,
                row.unit_price,
                row.amount,
                row.remark,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_items_by_statement_id(pool: &PgPool, statement_id: i64) -> Result<Vec<ReconciliationItem>> {
        let items = sqlx::query_as::<_, ReconciliationItem>(
            "SELECT item_id, statement_id, source_type, source_id, product_id, \
             product_code, product_name, unit, quantity, unit_price, amount, remark, created_at \
             FROM reconciliation_items WHERE statement_id = $1 ORDER BY item_id",
        )
        .bind(statement_id)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    pub async fn delete_adjustments_by_statement(executor: Executor<'_>, statement_id: i64) -> Result<()> {
        sqlx::query!(
            "DELETE FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'adjustment'",
            statement_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn recalculate_totals(executor: Executor<'_>, statement_id: i64) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE reconciliation_statements SET
                shipping_total = COALESCE((SELECT SUM(amount) FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'shipping'), 0),
                return_total = COALESCE((SELECT SUM(ABS(amount)) FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'return'), 0),
                adjustment_total = COALESCE((SELECT SUM(amount) FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'adjustment'), 0),
                net_amount = COALESCE((SELECT SUM(amount) FROM reconciliation_items WHERE statement_id = $1), 0),
                updated_at = NOW()
            WHERE statement_id = $1
            "#,
            statement_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }
}
