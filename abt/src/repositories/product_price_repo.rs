//! 产品价格数据访问层
//!
//! 提供产品价格和历史记录的数据库操作。
//! 使用 product_price 表替代原 product_price_log。

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::repositories::{Executor, PaginatedResult, PaginationParams};
use crate::service::{AllPriceHistoryQuery, PriceLogEntry, PriceLogWithProduct};

/// 产品价格数据仓库
pub struct ProductPriceRepo;

impl ProductPriceRepo {
    /// 获取产品当前价格（product_price 最新记录）
    pub async fn get_price(pool: &PgPool, product_id: i64) -> Result<Option<Decimal>> {
        let price: Option<Decimal> = sqlx::query_scalar!(
            "SELECT price FROM product_price WHERE product_id = $1 ORDER BY created_at DESC LIMIT 1",
            product_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(price)
    }

    /// 更新产品价格（INSERT 到 product_price 表，最新行即当前价格）
    pub async fn update_price(
        executor: Executor<'_>,
        product_id: i64,
        new_price: Decimal,
        operator_id: Option<i64>,
        remark: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            "INSERT INTO product_price (product_id, price, operator_id, remark) VALUES ($1, $2, $3, $4)",
            product_id,
            new_price,
            operator_id,
            remark
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 统计价格历史记录数
    pub async fn count_price_history(pool: &PgPool, product_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM product_price WHERE product_id = $1",
            product_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

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

        let total = Self::count_price_history(pool, product_id).await?;

        let items = sqlx::query_as::<_, PriceLogEntry>(
            "SELECT id as log_id, product_id, NULL as old_price, price as new_price, operator_id, remark, created_at
             FROM product_price
             WHERE product_id = $1
             ORDER BY created_at DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(product_id)
        .bind(page_size as i64)
        .bind(offset as i64)
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
            FROM product_price pp
            JOIN products p ON pp.product_id = p.product_id
            WHERE ($1::bigint IS NULL OR pp.product_id = $1)
              AND ($2::text IS NULL OR p.pdt_name ILIKE '%' || $2 || '%')
              AND ($3::text IS NULL OR p.product_code ILIKE '%' || $3 || '%')"#,
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

        let total = Self::count_all_price_history(
            pool,
            query.product_id,
            query.product_name.as_deref(),
            query.product_code.as_deref(),
        )
        .await?;

        let items = sqlx::query_as::<_, PriceLogWithProduct>(
            r#"SELECT
                pp.id as log_id,
                pp.product_id,
                p.pdt_name as product_name,
                p.product_code as product_code,
                NULL as old_price,
                pp.price as new_price,
                pp.operator_id,
                pp.remark,
                pp.created_at
            FROM product_price pp
            JOIN products p ON pp.product_id = p.product_id
            WHERE ($1::bigint IS NULL OR pp.product_id = $1)
              AND ($2::text IS NULL OR p.pdt_name ILIKE '%' || $2 || '%')
              AND ($3::text IS NULL OR p.product_code ILIKE '%' || $3 || '%')
            ORDER BY pp.created_at DESC
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

    /// 批量获取多个产品的当前价格
    pub async fn get_prices_by_ids(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, rust_decimal::Decimal>> {
        use std::collections::HashMap;
        use rust_decimal::Decimal;

        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            "SELECT DISTINCT ON (product_id) product_id, price
             FROM product_price
             WHERE product_id = ANY($1)
             ORDER BY product_id, created_at DESC",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;

        let mut map = HashMap::new();
        for row in rows {
            let product_id: i64 = sqlx::Row::try_get(&row, "product_id")?;
            let price: Decimal = sqlx::Row::try_get(&row, "price")?;
            map.insert(product_id, price);
        }
        Ok(map)
    }
}
