use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{SalesReturn, SalesReturnItem, SalesReturnQuery};
use crate::repositories::{build_fuzzy_pattern, Executor, PaginationParams};

pub struct SalesReturnInsertParams<'a> {
    pub return_no: &'a str,
    pub request_id: i64,
    pub order_id: i64,
    pub customer_name: &'a str,
    pub total_amount: Decimal,
    pub remark: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub operator_id: Option<i64>,
}

pub struct SalesReturnUpdateParams<'a> {
    pub remark: Option<&'a str>,
    pub reason: Option<&'a str>,
}

pub struct SalesReturnItemRow<'a> {
    pub request_item_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub product_code: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<&'a str>,
}

pub struct SalesReturnRepo;

impl SalesReturnRepo {
    pub async fn insert(executor: Executor<'_>, p: &SalesReturnInsertParams<'_>) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO sales_returns (return_no, request_id, order_id, customer_name, total_amount, remark, reason, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING return_id
            "#,
            p.return_no,
            p.request_id,
            p.order_id,
            p.customer_name,
            p.total_amount,
            p.remark,
            p.reason,
            p.operator_id,
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(executor: Executor<'_>, return_id: i64, p: &SalesReturnUpdateParams<'_>) -> Result<()> {
        sqlx::query!(
            "UPDATE sales_returns SET remark = $1, reason = $2, updated_at = NOW() WHERE return_id = $3",
            p.remark,
            p.reason,
            return_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn soft_delete(executor: Executor<'_>, return_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE sales_returns SET deleted_at = NOW() WHERE return_id = $1",
            return_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_status(executor: Executor<'_>, return_id: i64, status: i16) -> Result<()> {
        sqlx::query!(
            "UPDATE sales_returns SET status = $1, updated_at = NOW() WHERE return_id = $2",
            status,
            return_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, return_id: i64) -> Result<Option<SalesReturn>> {
        let row = sqlx::query_as::<_, SalesReturn>(
            "SELECT return_id, return_no, request_id, order_id, customer_name, \
             status, total_amount, remark, reason, operator_id, created_at, updated_at, deleted_at \
             FROM sales_returns WHERE return_id = $1 AND deleted_at IS NULL",
        )
        .bind(return_id)
        .fetch_optional(pool)
        .await?;

        if let Some(mut r) = row {
            r.items = Self::find_items_by_return_id(pool, return_id).await?;
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }

    pub async fn find_status(pool: &PgPool, return_id: i64) -> Result<Option<i16>> {
        let row: Option<(i16,)> = sqlx::query_as(
            "SELECT status FROM sales_returns WHERE return_id = $1 AND deleted_at IS NULL",
        )
        .bind(return_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn query(pool: &PgPool, q: &SalesReturnQuery) -> Result<(Vec<SalesReturn>, i64)> {
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
                    "(return_no ILIKE ${} OR customer_name ILIKE ${})",
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

        let order_id_param = if let Some(oid) = q.order_id {
            param_idx += 1;
            where_clauses.push(format!("order_id = ${}", param_idx));
            Some(oid)
        } else {
            None
        };

        let request_id_param = if let Some(rid) = q.request_id {
            param_idx += 1;
            where_clauses.push(format!("request_id = ${}", param_idx));
            Some(rid)
        } else {
            None
        };

        let where_sql = where_clauses.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM sales_returns WHERE {}", where_sql);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(ref p) = keyword_param { count_query = count_query.bind(p); }
        if let Some(s) = status_param { count_query = count_query.bind(s); }
        if let Some(oid) = order_id_param { count_query = count_query.bind(oid); }
        if let Some(rid) = request_id_param { count_query = count_query.bind(rid); }
        let total = count_query.fetch_one(pool).await?;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT return_id, return_no, request_id, order_id, customer_name, \
             status, total_amount, remark, reason, operator_id, created_at, updated_at, deleted_at \
             FROM sales_returns WHERE {} ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            where_sql, limit_idx, offset_idx
        );

        let mut data_query = sqlx::query_as::<_, SalesReturn>(&data_sql);
        if let Some(ref p) = keyword_param { data_query = data_query.bind(p); }
        if let Some(s) = status_param { data_query = data_query.bind(s); }
        if let Some(oid) = order_id_param { data_query = data_query.bind(oid); }
        if let Some(rid) = request_id_param { data_query = data_query.bind(rid); }
        data_query = data_query
            .bind(pagination.page_size as i64)
            .bind(pagination.offset() as i64);

        let items = data_query.fetch_all(pool).await?;
        Ok((items, total))
    }

    pub async fn insert_items(
        executor: Executor<'_>,
        return_id: i64,
        items: &[SalesReturnItemRow<'_>],
    ) -> Result<()> {
        for row in items {
            sqlx::query!(
                r#"
                INSERT INTO sales_return_items (return_id, request_item_id, order_item_id, product_id, product_code, product_name, unit, unit_price, quantity, subtotal, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
                return_id,
                row.request_item_id,
                row.order_item_id,
                row.product_id,
                row.product_code,
                row.product_name,
                row.unit,
                row.unit_price,
                row.quantity,
                row.subtotal,
                row.remark,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn delete_by_return(executor: Executor<'_>, return_id: i64) -> Result<()> {
        sqlx::query!(
            "DELETE FROM sales_return_items WHERE return_id = $1",
            return_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_items_by_return_id(pool: &PgPool, return_id: i64) -> Result<Vec<SalesReturnItem>> {
        let items = sqlx::query_as::<_, SalesReturnItem>(
            "SELECT item_id, return_id, request_item_id, order_item_id, product_id, \
             product_code, product_name, unit, unit_price, quantity, subtotal, remark, created_at \
             FROM sales_return_items WHERE return_id = $1 ORDER BY item_id",
        )
        .bind(return_id)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
