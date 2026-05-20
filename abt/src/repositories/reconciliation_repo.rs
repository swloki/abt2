use anyhow::Result;
use sqlx::PgPool;

use crate::models::{ReconciliationItem, ReconciliationQuery, ReconciliationStatement};
use crate::repositories::{build_fuzzy_pattern, Executor};

pub struct ReconciliationRepo;

impl ReconciliationRepo {
    pub async fn insert(executor: Executor<'_>, stmt: &ReconciliationStatement) -> Result<i64> {
        let statement_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO reconciliation_statements (statement_no, customer_name, period_year, period_month, shipping_total, return_total, adjustment_total, net_amount, status, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING statement_id
            "#,
        )
        .bind(&stmt.statement_no)
        .bind(&stmt.customer_name)
        .bind(stmt.period_year)
        .bind(stmt.period_month)
        .bind(stmt.shipping_total)
        .bind(stmt.return_total)
        .bind(stmt.adjustment_total)
        .bind(stmt.net_amount)
        .bind(stmt.status)
        .bind(&stmt.remark)
        .bind(stmt.operator_id)
        .fetch_one(executor)
        .await?;

        Ok(statement_id)
    }

    pub async fn update(executor: Executor<'_>, stmt: &ReconciliationStatement) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE reconciliation_statements
            SET remark = $1, updated_at = NOW()
            WHERE statement_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(&stmt.remark)
        .bind(stmt.statement_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn soft_delete(executor: Executor<'_>, statement_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE reconciliation_statements SET deleted_at = NOW() WHERE statement_id = $1 AND deleted_at IS NULL",
        )
        .bind(statement_id)
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

        Ok(row)
    }

    pub async fn find_by_period(pool: &PgPool, customer_name: &str, year: i16, month: i16) -> Result<Option<ReconciliationStatement>> {
        let row = sqlx::query_as::<_, ReconciliationStatement>(
            "SELECT statement_id, statement_no, customer_name, period_year, period_month, \
             shipping_total, return_total, adjustment_total, net_amount, status, remark, \
             operator_id, created_at, updated_at, deleted_at \
             FROM reconciliation_statements \
             WHERE customer_name = $1 AND period_year = $2 AND period_month = $3 AND deleted_at IS NULL",
        )
        .bind(customer_name)
        .bind(year)
        .bind(month)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    pub async fn query(pool: &PgPool, q: &ReconciliationQuery) -> Result<Vec<ReconciliationStatement>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT statement_id, statement_no, customer_name, period_year, period_month, \
             shipping_total, return_total, adjustment_total, net_amount, status, remark, \
             operator_id, created_at, updated_at, deleted_at \
             FROM reconciliation_statements WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (statement_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(year) = q.period_year {
            qb.push(" AND period_year = ");
            qb.push_bind(year);
        }

        if let Some(month) = q.period_month {
            qb.push(" AND period_month = ");
            qb.push_bind(month);
        }

        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(12).clamp(1, 100);

        qb.push(" ORDER BY statement_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<ReconciliationStatement>().fetch_all(pool).await?;
        Ok(result)
    }

    pub async fn query_count(pool: &PgPool, q: &ReconciliationQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM reconciliation_statements WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (statement_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(year) = q.period_year {
            qb.push(" AND period_year = ");
            qb.push_bind(year);
        }

        if let Some(month) = q.period_month {
            qb.push(" AND period_month = ");
            qb.push_bind(month);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    pub async fn update_status(executor: Executor<'_>, statement_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE reconciliation_statements SET status = $1, updated_at = NOW() WHERE statement_id = $2 AND deleted_at IS NULL",
        )
        .bind(status)
        .bind(statement_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn update_totals(executor: Executor<'_>, statement_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE reconciliation_statements
            SET shipping_total = (SELECT COALESCE(SUM(amount), 0) FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'shipping'),
                return_total = (SELECT COALESCE(SUM(amount), 0) FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'return'),
                adjustment_total = (SELECT COALESCE(SUM(amount), 0) FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'adjustment'),
                net_amount = (SELECT COALESCE(SUM(CASE WHEN source_type = 'return' THEN -amount ELSE amount END), 0) FROM reconciliation_items WHERE statement_id = $1),
                updated_at = NOW()
            WHERE statement_id = $1
            "#,
        )
        .bind(statement_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    // === 行项目 ===

    pub async fn insert_items(executor: Executor<'_>, items: &[ReconciliationItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO reconciliation_items (statement_id, source_type, source_id, product_id, product_code, product_name, unit, quantity, unit_price, amount, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(item.statement_id)
            .bind(&item.source_type)
            .bind(item.source_id)
            .bind(item.product_id)
            .bind(&item.product_code)
            .bind(&item.product_name)
            .bind(&item.unit)
            .bind(item.quantity)
            .bind(item.unit_price)
            .bind(item.amount)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    pub async fn delete_by_statement(executor: Executor<'_>, statement_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM reconciliation_items WHERE statement_id = $1 AND source_type = 'adjustment'")
            .bind(statement_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_statement_id(pool: &PgPool, statement_id: i64) -> Result<Vec<ReconciliationItem>> {
        let rows = sqlx::query_as::<_, ReconciliationItem>(
            "SELECT item_id, statement_id, source_type, source_id, product_id, product_code, \
             product_name, unit, quantity, unit_price, amount, remark, created_at \
             FROM reconciliation_items WHERE statement_id = $1 ORDER BY item_id",
        )
        .bind(statement_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 查询指定客户指定月份已发货的发货单行项目明细
    pub async fn query_shipping_items(pool: &PgPool, customer_name: &str, year: i16, month: i16) -> Result<Vec<ReconciliationItem>> {
        let rows = sqlx::query_as::<_, ReconciliationItem>(
            r#"
            SELECT 0::bigint AS item_id, 0::bigint AS statement_id,
                   'shipping'::varchar AS source_type,
                   sr.request_id AS source_id,
                   sri.product_id,
                   sri.product_code,
                   sri.product_name,
                   sri.unit,
                   sri.quantity,
                   soi.unit_price,
                   (sri.quantity * soi.unit_price) AS amount,
                   NULL::text AS remark,
                   NOW() AS created_at
            FROM shipping_requests sr
            JOIN shipping_request_items sri ON sr.request_id = sri.request_id
            JOIN sales_order_items soi ON sri.order_item_id = soi.item_id
            WHERE sr.customer_name = $1
              AND sr.status = 3
              AND sr.deleted_at IS NULL
              AND EXTRACT(YEAR FROM sr.shipped_at)::smallint = $2
              AND EXTRACT(MONTH FROM sr.shipped_at)::smallint = $3
            "#,
        )
        .bind(customer_name)
        .bind(year)
        .bind(month)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 查询指定客户指定月份已完成的退货单行项目明细
    pub async fn query_return_items(pool: &PgPool, customer_name: &str, year: i16, month: i16) -> Result<Vec<ReconciliationItem>> {
        let rows = sqlx::query_as::<_, ReconciliationItem>(
            r#"
            SELECT 0::bigint AS item_id, 0::bigint AS statement_id,
                   'return'::varchar AS source_type,
                   sret.return_id AS source_id,
                   sri.product_id,
                   sri.product_code,
                   sri.product_name,
                   sri.unit,
                   sri.quantity,
                   sri.unit_price,
                   sri.subtotal AS amount,
                   NULL::text AS remark,
                   NOW() AS created_at
            FROM sales_returns sret
            JOIN sales_return_items sri ON sret.return_id = sri.return_id
            WHERE sret.customer_name = $1
              AND sret.status = 4
              AND sret.deleted_at IS NULL
              AND EXTRACT(YEAR FROM sret.updated_at)::smallint = $2
              AND EXTRACT(MONTH FROM sret.updated_at)::smallint = $3
            "#,
        )
        .bind(customer_name)
        .bind(year)
        .bind(month)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
