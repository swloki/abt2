use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{ShippingRequest, ShippingRequestItem, ShippingRequestQuery};
use crate::repositories::{build_fuzzy_pattern, Executor, PaginationParams};

pub struct ShippingRequestInsertParams<'a> {
    pub request_no: &'a str,
    pub order_id: i64,
    pub customer_name: &'a str,
    pub remark: Option<&'a str>,
    pub operator_id: Option<i64>,
}

pub struct ShippingRequestUpdateParams<'a> {
    pub remark: Option<&'a str>,
}

pub struct ShippingRequestItemRow<'a> {
    pub order_item_id: i64,
    pub product_id: i64,
    pub product_code: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub quantity: Decimal,
    pub remark: Option<&'a str>,
}

pub struct ShippingRequestRepo;

impl ShippingRequestRepo {
    pub async fn insert(
        executor: Executor<'_>,
        p: &ShippingRequestInsertParams<'_>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO shipping_requests (request_no, order_id, customer_name, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING request_id
            "#,
            p.request_no,
            p.order_id,
            p.customer_name,
            p.remark,
            p.operator_id,
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(
        executor: Executor<'_>,
        request_id: i64,
        p: &ShippingRequestUpdateParams<'_>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE shipping_requests
            SET remark = $1, updated_at = NOW()
            WHERE request_id = $2
            "#,
            p.remark,
            request_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn soft_delete(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE shipping_requests SET deleted_at = NOW() WHERE request_id = $1",
            request_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_status(
        executor: Executor<'_>,
        request_id: i64,
        status: i16,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE shipping_requests SET status = $1, updated_at = NOW() WHERE request_id = $2",
            status,
            request_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_confirmed_at(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE shipping_requests SET confirmed_at = NOW(), updated_at = NOW() WHERE request_id = $1",
            request_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_shipped_at(executor: Executor<'_>, request_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE shipping_requests SET shipped_at = NOW(), updated_at = NOW() WHERE request_id = $1",
            request_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, request_id: i64) -> Result<Option<ShippingRequest>> {
        let row = sqlx::query_as::<_, ShippingRequest>(
            "SELECT request_id, request_no, order_id, customer_name, status, remark, \
             operator_id, confirmed_at, shipped_at, created_at, updated_at \
             FROM shipping_requests WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .fetch_optional(pool)
        .await?;

        if let Some(mut r) = row {
            r.items = Self::find_items_by_request_id(pool, request_id).await?;
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }

    pub async fn find_status(pool: &PgPool, request_id: i64) -> Result<Option<i16>> {
        let row: Option<(i16,)> = sqlx::query_as(
            "SELECT status FROM shipping_requests WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn find_order_id(pool: &PgPool, request_id: i64) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT order_id FROM shipping_requests WHERE request_id = $1 AND deleted_at IS NULL",
        )
        .bind(request_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn query(
        pool: &PgPool,
        q: &ShippingRequestQuery,
    ) -> Result<(Vec<ShippingRequest>, i64)> {
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
                    "(request_no ILIKE ${} OR customer_name ILIKE ${})",
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

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql =
            format!("SELECT COUNT(*) as count FROM shipping_requests WHERE {}", where_sql);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(ref p) = keyword_param {
            count_query = count_query.bind(p);
        }
        if let Some(s) = status_param {
            count_query = count_query.bind(s);
        }
        if let Some(oid) = order_id_param {
            count_query = count_query.bind(oid);
        }
        let total = count_query.fetch_one(pool).await?;

        // Data
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT request_id, request_no, order_id, customer_name, status, remark, \
             operator_id, confirmed_at, shipped_at, created_at, updated_at \
             FROM shipping_requests WHERE {} ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            where_sql, limit_idx, offset_idx
        );

        let mut data_query = sqlx::query_as::<_, ShippingRequest>(&data_sql);
        if let Some(ref p) = keyword_param {
            data_query = data_query.bind(p);
        }
        if let Some(s) = status_param {
            data_query = data_query.bind(s);
        }
        if let Some(oid) = order_id_param {
            data_query = data_query.bind(oid);
        }
        data_query = data_query
            .bind(pagination.page_size as i64)
            .bind(pagination.offset() as i64);

        let items = data_query.fetch_all(pool).await?;
        Ok((items, total))
    }

    pub async fn insert_items(
        executor: Executor<'_>,
        request_id: i64,
        items: &[ShippingRequestItemRow<'_>],
    ) -> Result<()> {
        for row in items {
            sqlx::query!(
                r#"
                INSERT INTO shipping_request_items (request_id, order_item_id, product_id, product_code, product_name, unit, quantity, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                request_id,
                row.order_item_id,
                row.product_id,
                row.product_code,
                row.product_name,
                row.unit,
                row.quantity,
                row.remark,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn delete_by_request(
        executor: Executor<'_>,
        request_id: i64,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM shipping_request_items WHERE request_id = $1",
            request_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_items_by_request_id(
        pool: &PgPool,
        request_id: i64,
    ) -> Result<Vec<ShippingRequestItem>> {
        let items = sqlx::query_as::<_, ShippingRequestItem>(
            "SELECT item_id, request_id, order_item_id, product_id, product_code, product_name, \
             unit, quantity, remark, created_at \
             FROM shipping_request_items WHERE request_id = $1 ORDER BY item_id",
        )
        .bind(request_id)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
