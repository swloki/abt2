use anyhow::Result;
use sqlx::PgPool;

use crate::models::{SalesReturn, SalesReturnItem, SalesReturnQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

pub struct SalesReturnRepo;

impl SalesReturnRepo {
    pub async fn insert(executor: Executor<'_>, ret: &SalesReturn) -> Result<i64> {
        let return_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO sales_returns (return_no, request_id, order_id, customer_name, status, total_amount, remark, reason, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING return_id
            "#,
        )
        .bind(&ret.return_no)
        .bind(ret.request_id)
        .bind(ret.order_id)
        .bind(&ret.customer_name)
        .bind(ret.status)
        .bind(ret.total_amount)
        .bind(&ret.remark)
        .bind(&ret.reason)
        .bind(ret.operator_id)
        .fetch_one(executor)
        .await?;

        Ok(return_id)
    }

    pub async fn update(executor: Executor<'_>, ret: &SalesReturn) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sales_returns
            SET remark = $1, reason = $2, updated_at = NOW()
            WHERE return_id = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(&ret.remark)
        .bind(&ret.reason)
        .bind(ret.return_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn soft_delete(executor: Executor<'_>, return_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE sales_returns SET deleted_at = NOW() WHERE return_id = $1 AND deleted_at IS NULL",
        )
        .bind(return_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, return_id: i64) -> Result<Option<SalesReturn>> {
        let row = sqlx::query_as::<_, SalesReturn>(
            "SELECT return_id, return_no, request_id, order_id, customer_name, status, total_amount, \
             remark, reason, operator_id, created_at, updated_at, deleted_at \
             FROM sales_returns WHERE return_id = $1 AND deleted_at IS NULL",
        )
        .bind(return_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    pub async fn query(pool: &PgPool, q: &SalesReturnQuery) -> Result<Vec<SalesReturn>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT return_id, return_no, request_id, order_id, customer_name, status, total_amount, \
             remark, reason, operator_id, created_at, updated_at, deleted_at \
             FROM sales_returns WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (return_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(order_id) = q.order_id {
            qb.push(" AND order_id = ");
            qb.push_bind(order_id);
        }

        if let Some(request_id) = q.request_id {
            qb.push(" AND request_id = ");
            qb.push_bind(request_id);
        }

        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(12).clamp(1, 100);

        qb.push(" ORDER BY return_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<SalesReturn>().fetch_all(pool).await?;
        Ok(result)
    }

    pub async fn query_count(pool: &PgPool, q: &SalesReturnQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM sales_returns WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (return_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(order_id) = q.order_id {
            qb.push(" AND order_id = ");
            qb.push_bind(order_id);
        }

        if let Some(request_id) = q.request_id {
            qb.push(" AND request_id = ");
            qb.push_bind(request_id);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    pub async fn update_status(executor: Executor<'_>, return_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE sales_returns SET status = $1, updated_at = NOW() WHERE return_id = $2 AND deleted_at IS NULL",
        )
        .bind(status)
        .bind(return_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    // === 行项目 ===

    pub async fn insert_items(executor: Executor<'_>, items: &[SalesReturnItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO sales_return_items (return_id, request_item_id, order_item_id, product_id, product_code, product_name, unit, unit_price, quantity, subtotal, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(item.return_id)
            .bind(item.request_item_id)
            .bind(item.order_item_id)
            .bind(item.product_id)
            .bind(&item.product_code)
            .bind(&item.product_name)
            .bind(&item.unit)
            .bind(item.unit_price)
            .bind(item.quantity)
            .bind(item.subtotal)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    pub async fn delete_by_return(executor: Executor<'_>, return_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM sales_return_items WHERE return_id = $1")
            .bind(return_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_return_id(pool: &PgPool, return_id: i64) -> Result<Vec<SalesReturnItem>> {
        let rows = sqlx::query_as::<_, SalesReturnItem>(
            "SELECT item_id, return_id, request_item_id, order_item_id, product_id, product_code, \
             product_name, unit, unit_price, quantity, subtotal, remark, created_at \
             FROM sales_return_items WHERE return_id = $1 ORDER BY item_id",
        )
        .bind(return_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 汇总指定 order_item_id 的所有有效退货数量（含 Pending/Approved/Received/Completed）
    pub async fn sum_returned_qty(pool: &PgPool, order_item_id: i64) -> Result<rust_decimal::Decimal> {
        let qty: rust_decimal::Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(sri.quantity), 0)
            FROM sales_return_items sri
            JOIN sales_returns sr ON sri.return_id = sr.return_id
            WHERE sri.order_item_id = $1
              AND sr.status IN (1, 2, 3, 4)
              AND sr.deleted_at IS NULL
            "#,
        )
        .bind(order_item_id)
        .fetch_one(pool)
        .await?;

        Ok(qty)
    }
}
