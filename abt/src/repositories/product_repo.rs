//! 产品数据访问层
//!
//! 提供产品的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{Product, ProductQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

/// 产品数据仓库
pub struct ProductRepo;

impl ProductRepo {
    /// 创建新产品
    pub async fn insert(
        executor: Executor<'_>,
        pdt_name: &str,
        product_code: &str,
        unit: &str,
        meta: crate::models::ProductMeta,
    ) -> Result<i64> {
        let product_id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO products (pdt_name, product_code, unit, meta)
            VALUES ($1, $2, $3, $4::jsonb)
            RETURNING product_id
            "#,
            pdt_name,
            product_code,
            unit,
            serde_json::json!(meta)
        )
        .fetch_one(executor)
        .await?;

        Ok(product_id)
    }

    /// 更新产品
    pub async fn update(
        executor: Executor<'_>,
        product_id: i64,
        pdt_name: &str,
        product_code: &str,
        unit: &str,
        meta: crate::models::ProductMeta,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE products
            SET pdt_name = $1, product_code = $2, unit = $3, meta = $4::jsonb
            WHERE product_id = $5
            "#,
            pdt_name,
            product_code,
            unit,
            serde_json::json!(meta),
            product_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 删除产品
    pub async fn delete(executor: Executor<'_>, product_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM products WHERE product_id = $1", product_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 根据 ID 查找产品
    #[allow(dead_code)]
    pub async fn find_by_id(pool: &PgPool, product_id: i64) -> Result<Option<Product>> {
        let row = sqlx::query_as::<_, Product>(
            "SELECT product_id, pdt_name, product_code, unit, meta FROM products WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 查询产品列表
    #[allow(dead_code)]
    pub async fn query(pool: &PgPool, query: &ProductQuery) -> Result<Vec<Product>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT DISTINCT p.product_id, p.pdt_name, p.product_code, p.unit, p.meta FROM products p",
        );

        // term_id 过滤: 通过 term_relation 关联表 JOIN 查询
        if let Some(term_id) = query.term_id {
            qb.push(" JOIN term_relation tr ON p.product_id = tr.product_id AND tr.term_id = ");
            qb.push_bind(term_id);
        }

        qb.push(" WHERE 1=1");

        if let Some(pdt_name) = &query.pdt_name
            && !pdt_name.is_empty()
            && let Some(pattern) = build_fuzzy_pattern(pdt_name) {
                qb.push(" AND p.pdt_name ILIKE ");
                qb.push_bind(pattern);
            }

        if let Some(product_code) = &query.product_code
            && !product_code.is_empty()
        {
            qb.push(" AND p.product_code ILIKE ");
            qb.push_bind(format!("%{}%", product_code));
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(12).clamp(1, 100);

        qb.push(" ORDER BY p.product_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<Product>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询产品总数
    #[allow(dead_code)]
    pub async fn query_count(pool: &PgPool, query: &ProductQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new("SELECT count(*) FROM products p");

        if let Some(pdt_name) = &query.pdt_name
            && !pdt_name.is_empty()
        {
            if let Some(pattern) = build_fuzzy_pattern(pdt_name) {
                qb.push(" WHERE p.pdt_name ILIKE ");
                qb.push_bind(pattern);
            } else {
                qb.push(" WHERE 1=1");
            }
        } else {
            qb.push(" WHERE 1=1");
        }

        if let Some(product_code) = &query.product_code
            && !product_code.is_empty()
        {
            qb.push(" AND p.product_code ILIKE ");
            qb.push_bind(format!("%{}%", product_code));
        }

        // term_id 过滤: 通过 term_relation 关联表 JOIN
        if let Some(term_id) = query.term_id {
            qb.push(" AND EXISTS (SELECT 1 FROM term_relation tr WHERE tr.product_id = p.product_id AND tr.term_id = ");
            qb.push_bind(term_id);
            qb.push(")");
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 检查产品编码是否存在
    pub async fn exist_product_code(pool: &PgPool, code: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM products WHERE product_code = $1",
            code
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        Ok(count > 0)
    }

    /// 根据产品编码查找产品
    pub async fn find_by_code(pool: &PgPool, code: &str) -> Result<Option<Product>> {
        let row = sqlx::query_as::<_, Product>(
            "SELECT product_id, pdt_name, product_code, unit, meta FROM products WHERE product_code = $1",
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 根据产品 ID 列表批量查询产品
    pub async fn find_by_ids(pool: &PgPool, product_ids: &[i64]) -> Result<Vec<Product>> {
        if product_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, Product>(
            "SELECT product_id, pdt_name, product_code, unit, meta FROM products WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 根据产品编码列表批量查询产品
    pub async fn find_by_codes(pool: &PgPool, codes: &[String]) -> Result<Vec<Product>> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, Product>(
            "SELECT product_id, pdt_name, product_code, unit, meta FROM products WHERE product_code = ANY($1)",
        )
        .bind(codes)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 更新产品名称
    pub async fn update_name(
        executor: Executor<'_>,
        product_id: i64,
        pdt_name: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE products
            SET pdt_name = $1
            WHERE product_id = $2
            "#,
            pdt_name,
            product_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }
}
