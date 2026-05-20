//! 报价单数据访问层
//!
//! 提供报价单主表及行项目的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{Quotation, QuotationItem, QuotationQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

/// 报价单数据仓库
pub struct QuotationRepo;

impl QuotationRepo {
    // === Main table ===

    /// 创建报价单，返回 quotation_id
    pub async fn insert(executor: Executor<'_>, q: &Quotation) -> Result<i64> {
        let quotation_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO quotations (quotation_no, customer_name, contact_person, contact_phone, status, total_amount, remark, valid_until, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING quotation_id
            "#,
        )
        .bind(&q.quotation_no)
        .bind(&q.customer_name)
        .bind(&q.contact_person)
        .bind(&q.contact_phone)
        .bind(q.status)
        .bind(q.total_amount)
        .bind(&q.remark)
        .bind(q.valid_until)
        .bind(q.operator_id)
        .fetch_one(executor)
        .await?;

        Ok(quotation_id)
    }

    /// 更新报价单（不含 quotation_no 和 status）
    pub async fn update(executor: Executor<'_>, q: &Quotation) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE quotations
            SET customer_name = $1, contact_person = $2, contact_phone = $3, remark = $4, valid_until = $5, total_amount = $6, updated_at = NOW()
            WHERE quotation_id = $7 AND deleted_at IS NULL
            "#,
        )
        .bind(&q.customer_name)
        .bind(&q.contact_person)
        .bind(&q.contact_phone)
        .bind(&q.remark)
        .bind(q.valid_until)
        .bind(q.total_amount)
        .bind(q.quotation_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 软删除报价单
    pub async fn soft_delete(executor: Executor<'_>, quotation_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE quotations SET deleted_at = NOW() WHERE quotation_id = $1 AND deleted_at IS NULL",
        )
        .bind(quotation_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据 ID 查找报价单
    pub async fn find_by_id(pool: &PgPool, quotation_id: i64) -> Result<Option<Quotation>> {
        let row = sqlx::query_as::<_, Quotation>(
            "SELECT quotation_id, quotation_no, customer_name, contact_person, contact_phone, \
             status, total_amount, remark, valid_until, operator_id, created_at, updated_at, deleted_at \
             FROM quotations WHERE quotation_id = $1 AND deleted_at IS NULL",
        )
        .bind(quotation_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 查询报价单列表
    pub async fn query(pool: &PgPool, q: &QuotationQuery) -> Result<Vec<Quotation>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT quotation_id, quotation_no, customer_name, contact_person, contact_phone, \
             status, total_amount, remark, valid_until, operator_id, created_at, updated_at, deleted_at \
             FROM quotations WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (quotation_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(12).clamp(1, 100);

        qb.push(" ORDER BY quotation_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<Quotation>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询报价单总数
    pub async fn query_count(pool: &PgPool, q: &QuotationQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM quotations WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &q.keyword
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (quotation_no ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR customer_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(status) = q.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新报价单状态
    pub async fn update_status(executor: Executor<'_>, quotation_id: i64, status: i16) -> Result<()> {
        sqlx::query(
            "UPDATE quotations SET status = $1, updated_at = NOW() WHERE quotation_id = $2 AND deleted_at IS NULL",
        )
        .bind(status)
        .bind(quotation_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    // === Line items ===

    /// 批量插入报价单行项目
    pub async fn insert_items(executor: Executor<'_>, items: &[QuotationItem]) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO quotation_items (quotation_id, product_id, product_code, product_name, unit, unit_price, quantity, discount, subtotal, remark)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(item.quotation_id)
            .bind(item.product_id)
            .bind(&item.product_code)
            .bind(&item.product_name)
            .bind(&item.unit)
            .bind(item.unit_price)
            .bind(item.quantity)
            .bind(item.discount)
            .bind(item.subtotal)
            .bind(&item.remark)
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    /// 删除报价单下的所有行项目
    pub async fn delete_by_quotation(executor: Executor<'_>, quotation_id: i64) -> Result<()> {
        sqlx::query(
            "DELETE FROM quotation_items WHERE quotation_id = $1",
        )
        .bind(quotation_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据报价单 ID 查询行项目
    pub async fn find_by_quotation_id(pool: &PgPool, quotation_id: i64) -> Result<Vec<QuotationItem>> {
        let rows = sqlx::query_as::<_, QuotationItem>(
            "SELECT item_id, quotation_id, product_id, product_code, product_name, unit, \
             unit_price, quantity, discount, subtotal, remark, created_at \
             FROM quotation_items WHERE quotation_id = $1 ORDER BY item_id",
        )
        .bind(quotation_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
