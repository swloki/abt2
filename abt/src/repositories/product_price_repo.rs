//! 产品价格数据访问层
//!
//! 提供产品价格和历史记录的数据库操作。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::repositories::{Executor, PaginatedResult, PaginationParams};
use crate::service::{AllPriceHistoryQuery, PriceLogEntry, PriceLogWithProduct};

/// 产品价格数据仓库
pub struct ProductPriceRepo;

impl ProductPriceRepo {
    /// 获取产品当前价格
    pub async fn get_price(executor: Executor<'_>, product_id: i64) -> Result<Option<Decimal>> {
        let price: Option<Decimal> = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT (meta->>'price')::decimal FROM products WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_optional(executor)
        .await?
        .flatten();

        Ok(price)
    }

    /// 更新产品价格
    pub async fn update_price(
        executor: Executor<'_>,
        product_id: i64,
        new_price: Decimal,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE products SET meta = jsonb_set(meta, '{price}', to_jsonb($1::decimal)) WHERE product_id = $2",
            new_price,
            product_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 记录价格变更历史
    pub async fn insert_price_log(
        executor: Executor<'_>,
        product_id: i64,
        old_price: Option<Decimal>,
        new_price: Decimal,
        operator_id: Option<i64>,
        remark: Option<&str>,
    ) -> Result<i64> {
        let log_id: i64 = sqlx::query_scalar!(
            "INSERT INTO product_price_log (product_id, old_price, new_price, operator_id, remark)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING log_id",
            product_id,
            old_price,
            new_price,
            operator_id,
            remark
        )
        .fetch_one(executor)
        .await?;

        Ok(log_id)
    }

    /// 统计价格历史记录数
    pub async fn count_price_history(pool: &PgPool, product_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM product_price_log WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// 分页查询价格历史
    pub async fn list_price_history(
        pool: &PgPool,
        product_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<PriceLogEntry>> {
        let offset = (page.saturating_sub(1)) * page_size;

        // 查询总数
        let total = Self::count_price_history(pool, product_id).await?;

        // 查询数据
        let items = sqlx::query_as!(
            PriceLogEntry,
            "SELECT log_id, product_id, old_price, new_price, operator_id, remark, created_at
             FROM product_price_log
             WHERE product_id = $1
             ORDER BY created_at DESC
             LIMIT $2 OFFSET $3",
            product_id,
            page_size as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    /// 统计所有价格历史记录数（可选按产品和名称/编码筛选）
    pub async fn count_all_price_history(
        pool: &PgPool,
        product_id: Option<i64>,
        product_name: Option<&str>,
        product_code: Option<&str>,
    ) -> Result<i64> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)
            FROM product_price_log ppl
            JOIN products p ON ppl.product_id = p.product_id
            WHERE ($1::bigint IS NULL OR ppl.product_id = $1)
              AND ($2::text IS NULL OR p.pdt_name ILIKE '%' || $2 || '%')
              AND ($3::text IS NULL OR p.meta->>'product_code' ILIKE '%' || $3 || '%')"#,
        )
        .bind(product_id)
        .bind(product_name)
        .bind(product_code)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// 分页查询所有产品的价格历史（包含产品信息）
    pub async fn list_all_price_history(
        pool: &PgPool,
        query: AllPriceHistoryQuery,
    ) -> Result<PaginatedResult<PriceLogWithProduct>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;
        let offset = (page.saturating_sub(1)) * page_size;

        // 查询总数
        let total = Self::count_all_price_history(
            pool,
            query.product_id,
            query.product_name.as_deref(),
            query.product_code.as_deref(),
        )
        .await?;

        // 查询数据
        let items = sqlx::query_as::<_, PriceLogWithProduct>(
            r#"SELECT
                ppl.log_id,
                ppl.product_id,
                p.pdt_name as product_name,
                p.meta->>'product_code' as product_code,
                ppl.old_price,
                ppl.new_price,
                ppl.operator_id,
                ppl.remark,
                ppl.created_at
            FROM product_price_log ppl
            JOIN products p ON ppl.product_id = p.product_id
            WHERE ($1::bigint IS NULL OR ppl.product_id = $1)
              AND ($2::text IS NULL OR p.pdt_name ILIKE '%' || $2 || '%')
              AND ($3::text IS NULL OR p.meta->>'product_code' ILIKE '%' || $3 || '%')
            ORDER BY ppl.created_at DESC
            LIMIT $4 OFFSET $5"#,
        )
        .bind(query.product_id)
        .bind(&query.product_name)
        .bind(&query.product_code)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }
}
