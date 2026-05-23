use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Quotation, QuotationItem, QuotationQuery};
use crate::repositories::{build_fuzzy_pattern, Executor, PaginationParams};

pub struct QuotationInsertParams<'a> {
    pub quotation_no: &'a str,
    pub customer_name: &'a str,
    pub contact_person: Option<&'a str>,
    pub contact_phone: Option<&'a str>,
    pub total_amount: Decimal,
    pub remark: Option<&'a str>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
    pub operator_id: Option<i64>,
}

pub struct QuotationUpdateParams<'a> {
    pub customer_name: &'a str,
    pub contact_person: Option<&'a str>,
    pub contact_phone: Option<&'a str>,
    pub total_amount: Decimal,
    pub remark: Option<&'a str>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct QuotationItemRow<'a> {
    pub product_id: i64,
    pub product_code: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub subtotal: Decimal,
    pub remark: Option<&'a str>,
}

pub struct QuotationRepo;

impl QuotationRepo {
    pub async fn insert(executor: Executor<'_>, p: &QuotationInsertParams<'_>) -> Result<i64> {
        let id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO quotations (quotation_no, customer_name, contact_person, contact_phone, total_amount, remark, valid_until, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING quotation_id
            "#,
            p.quotation_no,
            p.customer_name,
            p.contact_person,
            p.contact_phone,
            p.total_amount,
            p.remark,
            p.valid_until,
            p.operator_id,
        )
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(executor: Executor<'_>, quotation_id: i64, p: &QuotationUpdateParams<'_>) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE quotations
            SET customer_name = $1, contact_person = $2, contact_phone = $3,
                total_amount = $4, remark = $5, valid_until = $6, updated_at = NOW()
            WHERE quotation_id = $7
            "#,
            p.customer_name,
            p.contact_person,
            p.contact_phone,
            p.total_amount,
            p.remark,
            p.valid_until,
            quotation_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn soft_delete(executor: Executor<'_>, quotation_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE quotations SET deleted_at = NOW() WHERE quotation_id = $1",
            quotation_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_status(
        executor: Executor<'_>,
        quotation_id: i64,
        status: i16,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE quotations SET status = $1, updated_at = NOW() WHERE quotation_id = $2",
            status,
            quotation_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, quotation_id: i64) -> Result<Option<Quotation>> {
        let row = sqlx::query_as::<_, Quotation>(
            "SELECT quotation_id, quotation_no, customer_name, contact_person, contact_phone, \
             status, total_amount, remark, valid_until, operator_id, created_at, updated_at \
             FROM quotations WHERE quotation_id = $1 AND deleted_at IS NULL",
        )
        .bind(quotation_id)
        .fetch_optional(pool)
        .await?;

        if let Some(mut q) = row {
            q.items = Self::find_items_by_quotation_id(pool, quotation_id).await?;
            Ok(Some(q))
        } else {
            Ok(None)
        }
    }

    pub async fn find_status(pool: &PgPool, quotation_id: i64) -> Result<Option<i16>> {
        let row: Option<(i16,)> =
            sqlx::query_as("SELECT status FROM quotations WHERE quotation_id = $1 AND deleted_at IS NULL")
                .bind(quotation_id)
                .fetch_optional(pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn query(pool: &PgPool, q: &QuotationQuery) -> Result<(Vec<Quotation>, i64)> {
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
                    "(quotation_no ILIKE ${} OR customer_name ILIKE ${})",
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

        let where_sql = where_clauses.join(" AND ");

        // Count
        let count_sql = format!("SELECT COUNT(*) as count FROM quotations WHERE {}", where_sql);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some(ref p) = keyword_param {
            count_query = count_query.bind(p);
        }
        if let Some(s) = status_param {
            count_query = count_query.bind(s);
        }
        let total = count_query.fetch_one(pool).await?;

        // Data
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT quotation_id, quotation_no, customer_name, contact_person, contact_phone, \
             status, total_amount, remark, valid_until, operator_id, created_at, updated_at \
             FROM quotations WHERE {} ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            where_sql, limit_idx, offset_idx
        );

        let mut data_query = sqlx::query_as::<_, Quotation>(&data_sql);
        if let Some(ref p) = keyword_param {
            data_query = data_query.bind(p);
        }
        if let Some(s) = status_param {
            data_query = data_query.bind(s);
        }
        data_query = data_query
            .bind(pagination.page_size as i64)
            .bind(pagination.offset() as i64);

        let items = data_query.fetch_all(pool).await?;
        Ok((items, total))
    }

    pub async fn insert_items(
        executor: Executor<'_>,
        quotation_id: i64,
        items: &[QuotationItemRow<'_>],
    ) -> Result<()> {
        for row in items {
            sqlx::query!(
                r#"
                INSERT INTO quotation_items (quotation_id, product_id, product_code, product_name, unit, unit_price, quantity, discount, subtotal, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                quotation_id,
                row.product_id,
                row.product_code,
                row.product_name,
                row.unit,
                row.unit_price,
                row.quantity,
                row.discount,
                row.subtotal,
                row.remark,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn delete_by_quotation(
        executor: Executor<'_>,
        quotation_id: i64,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM quotation_items WHERE quotation_id = $1",
            quotation_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn find_items_by_quotation_id(
        pool: &PgPool,
        quotation_id: i64,
    ) -> Result<Vec<QuotationItem>> {
        let items = sqlx::query_as::<_, QuotationItem>(
            "SELECT item_id, quotation_id, product_id, product_code, product_name, unit, \
             unit_price, quantity, discount, subtotal, remark, created_at \
             FROM quotation_items WHERE quotation_id = $1 ORDER BY item_id",
        )
        .bind(quotation_id)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
