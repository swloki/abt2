//! 产品关注数据访问层

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{LowStockWatchedProduct, ProductWatcherUser, WatchedProductWithInventory};

pub struct ProductWatcherRepo;

impl ProductWatcherRepo {
    /// 关注产品（upsert），返回 is_new
    pub async fn upsert(
        pool: &PgPool,
        user_id: i64,
        product_id: i64,
        safety_stock_override: Option<Decimal>,
    ) -> Result<bool> {
        let row = sqlx::query_as::<_, (bool,)>(
            r#"
            INSERT INTO product_watchers (user_id, product_id, safety_stock_override, updated_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (user_id, product_id) DO UPDATE SET
                safety_stock_override = COALESCE(EXCLUDED.safety_stock_override, product_watchers.safety_stock_override),
                updated_at = now()
            RETURNING (xmax = 0) AS is_new
            "#,
        )
        .bind(user_id)
        .bind(product_id)
        .bind(safety_stock_override)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    /// 取消关注
    pub async fn delete(pool: &PgPool, user_id: i64, product_id: i64) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM product_watchers WHERE user_id = $1 AND product_id = $2",
        )
        .bind(user_id)
        .bind(product_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 查询用户关注的产品的数量
    pub async fn count_by_user(pool: &PgPool, user_id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM product_watchers WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    /// 查询用户关注的产品（含实时库存信息）
    pub async fn find_by_user_with_inventory(
        pool: &PgPool,
        user_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<WatchedProductWithInventory>, i64)> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total = Self::count_by_user(pool, user_id).await?;

        let items = sqlx::query_as::<sqlx::Postgres, WatchedProductWithInventory>(
            r#"
            SELECT
                pw.product_id,
                COALESCE(p.product_code, '') AS product_code,
                COALESCE(p.pdt_name, '') AS product_name,
                COALESCE(i.quantity, 0) AS current_quantity,
                COALESCE(pw.safety_stock_override, COALESCE(i.safety_stock, 0)) AS effective_safety_stock,
                (COALESCE(i.quantity, 0) < COALESCE(pw.safety_stock_override, COALESCE(i.safety_stock, 0))) AS is_alerting
            FROM product_watchers pw
            JOIN products p ON p.product_id = pw.product_id
            LEFT JOIN (
                SELECT product_id, SUM(quantity) as quantity, MAX(safety_stock) as safety_stock
                FROM inventory GROUP BY product_id
            ) i ON i.product_id = pw.product_id
            WHERE pw.user_id = $1
            ORDER BY pw.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        Ok((items, total))
    }

    /// 查询被关注的低库存产品（Worker 用）
    pub async fn find_watched_low_stock_products(
        pool: &PgPool,
    ) -> Result<Vec<LowStockWatchedProduct>> {
        let rows = sqlx::query_as::<_, LowStockWatchedProduct>(
            r#"
            SELECT
                pw.product_id,
                p.pdt_name AS product_name,
                i.quantity AS current_quantity,
                MIN(COALESCE(pw.safety_stock_override, i.safety_stock)) AS effective_safety_stock
            FROM product_watchers pw
            JOIN products p ON p.product_id = pw.product_id
            JOIN (
                SELECT product_id, SUM(quantity) as quantity, MAX(safety_stock) as safety_stock
                FROM inventory GROUP BY product_id
            ) i ON i.product_id = pw.product_id
            WHERE i.quantity < COALESCE(pw.safety_stock_override, i.safety_stock)
            GROUP BY pw.product_id, p.pdt_name, i.quantity
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 查询产品的关注者（Worker 用）
    pub async fn find_watchers_by_product(
        pool: &PgPool,
        product_id: i64,
    ) -> Result<Vec<ProductWatcherUser>> {
        let rows = sqlx::query_as::<_, ProductWatcherUser>(
            "SELECT user_id FROM product_watchers WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 批量查询多个产品的关注者（Worker 用）
    /// 返回 (product_id, Vec<ProductWatcherUser>) 对
    pub async fn find_watchers_by_products(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<Vec<(i64, i64)>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT product_id, user_id FROM product_watchers WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 批量查询关注者的告警状态（Worker 用）
    pub async fn batch_get_alert_status(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<Vec<(i64, i64, bool)>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
            "SELECT user_id, product_id, alert_active FROM product_watchers WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 批量设置告警状态为活跃（Worker 用）
    pub async fn batch_activate_alerts(
        pool: &PgPool,
        pairs: &[(i64, i64)],
    ) -> Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }
        let user_ids: Vec<i64> = pairs.iter().map(|(uid, _)| *uid).collect();
        let product_ids: Vec<i64> = pairs.iter().map(|(_, pid)| *pid).collect();
        sqlx::query(
            r#"UPDATE product_watchers
            SET alert_active = true, last_notified_at = now(), updated_at = now()
            WHERE (user_id, product_id) IN (
                SELECT * FROM UNNEST($1::bigint[], $2::bigint[])
            )"#,
        )
        .bind(&user_ids)
        .bind(&product_ids)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// 批量重置已回升产品的告警状态（Worker 用）
    pub async fn batch_clear_recovered(
        pool: &PgPool,
        low_stock_product_ids: &[i64],
    ) -> Result<u64> {
        if low_stock_product_ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query(
            r#"UPDATE product_watchers
            SET alert_active = false, updated_at = now()
            WHERE alert_active = true AND product_id != ALL($1)"#,
        )
        .bind(low_stock_product_ids)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}
