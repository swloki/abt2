//! 产品价格数据访问层
//!
//! 使用 product_price 表（最新行即当前价格）。

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::repositories::{Executor, PaginatedResult, PaginationParams};
use crate::service::{AllPriceHistoryQuery, PriceLogEntry, PriceLogWithProduct};

/// 产品价格数据仓库
pub struct ProductPriceRepo;

/// 价格历史查询结果行（含分页 total）
#[derive(sqlx::FromRow)]
struct PriceHistoryRow {
    log_id: i64,
    product_id: i64,
    new_price: Decimal,
    operator_id: Option<i64>,
    remark: Option<String>,
    created_at: DateTime<Utc>,
    total: i64,
}

/// 全量价格历史查询结果行（含产品信息和分页 total）
#[derive(sqlx::FromRow)]
struct AllPriceHistoryRow {
    log_id: i64,
    product_id: i64,
    product_name: String,
    product_code: Option<String>,
    new_price: Decimal,
    operator_id: Option<i64>,
    remark: Option<String>,
    created_at: DateTime<Utc>,
    total: i64,
}

impl ProductPriceRepo {
    /// 获取产品当前价格（product_price 最新记录）
    pub async fn get_price(executor: Executor<'_>, product_id: i64) -> Result<Option<Decimal>> {
        let price: Option<Decimal> = sqlx::query_scalar!(
            "SELECT price FROM product_price WHERE product_id = $1 ORDER BY created_at DESC LIMIT 1",
            product_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(price)
    }

    /// 更新产品价格（条件 INSERT：价格不同才插入）
    ///
    /// 使用 pg_advisory_xact_lock 防止并发写入导致重复行。
    /// 锁在事务结束时自动释放，无需显式 unlock。
    pub async fn update_price(
        executor: Executor<'_>,
        product_id: i64,
        new_price: Decimal,
        operator_id: Option<i64>,
        remark: Option<&str>,
    ) -> Result<()> {
        sqlx::query!("SELECT pg_advisory_xact_lock($1)", product_id)
            .execute(&mut *executor)
            .await?;

        sqlx::query!(
            r#"
            INSERT INTO product_price (product_id, price, operator_id, remark)
            SELECT $1, $2, $3, $4
            WHERE NOT EXISTS (
                SELECT 1 FROM product_price
                WHERE product_id = $1 AND price = $2
                ORDER BY created_at DESC LIMIT 1
            )
            "#,
            product_id,
            new_price,
            operator_id,
            remark
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 分页查询价格历史（COUNT(*) OVER() 合并总数到数据查询）
    pub async fn list_price_history(
        pool: &PgPool,
        product_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<PriceLogEntry>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let rows = sqlx::query_as::<_, PriceHistoryRow>(
            r#"SELECT id as log_id, product_id, price as new_price,
                      operator_id, remark, created_at,
                      COUNT(*) OVER() as total
               FROM product_price
               WHERE product_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(product_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        let total = rows.first().map(|r| r.total as u64).unwrap_or(0);
        let items: Vec<PriceLogEntry> = rows
            .into_iter()
            .map(|r| PriceLogEntry {
                log_id: r.log_id,
                product_id: r.product_id,
                new_price: r.new_price,
                operator_id: r.operator_id,
                remark: r.remark,
                created_at: r.created_at,
            })
            .collect();

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total, &pagination))
    }

    /// 分页查询所有产品的价格历史（包含产品信息，COUNT(*) OVER() 合并总数）
    pub async fn list_all_price_history(
        pool: &PgPool,
        query: AllPriceHistoryQuery,
    ) -> Result<PaginatedResult<PriceLogWithProduct>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;
        let offset = (page.saturating_sub(1)) * page_size;

        let rows = sqlx::query_as::<_, AllPriceHistoryRow>(
            r#"SELECT
                pp.id as log_id,
                pp.product_id,
                p.pdt_name as product_name,
                p.product_code as product_code,
                pp.price as new_price,
                pp.operator_id,
                pp.remark,
                pp.created_at,
                COUNT(*) OVER() as total
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

        let total = rows.first().map(|r| r.total as u64).unwrap_or(0);
        let items: Vec<PriceLogWithProduct> = rows
            .into_iter()
            .map(|r| PriceLogWithProduct {
                log_id: r.log_id,
                product_id: r.product_id,
                product_name: r.product_name,
                product_code: r.product_code,
                new_price: r.new_price,
                operator_id: r.operator_id,
                remark: r.remark,
                created_at: r.created_at,
            })
            .collect();

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total, &pagination))
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

        #[derive(sqlx::FromRow)]
        struct PriceRow {
            product_id: i64,
            price: Decimal,
        }

        let rows = sqlx::query_as::<_, PriceRow>(
            "SELECT DISTINCT ON (product_id) product_id, price
             FROM product_price
             WHERE product_id = ANY($1)
             ORDER BY product_id, created_at DESC",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.product_id, r.price)).collect())
    }
}
