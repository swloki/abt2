use common::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::RepoResult;

use super::model::{LowStockWatchedProduct, WatchedProductWithInventory};
use crate::shared::types::pagination::PaginatedResult;

pub struct ProductWatcherRepo;

impl ProductWatcherRepo {
    /// 关注产品（upsert），返回 is_new
    pub async fn upsert(
        executor: PgExecutor<'_>,
        user_id: i64,
        product_id: i64,
        safety_stock_override: Option<Decimal>,
    ) -> RepoResult<bool> {
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
        .fetch_one(executor)
        .await?;
        Ok(row.0)
    }

    /// 取消关注
    pub async fn delete(executor: PgExecutor<'_>, user_id: i64, product_id: i64) -> RepoResult<bool> {
        let result = sqlx::query(
            "DELETE FROM product_watchers WHERE user_id = $1 AND product_id = $2",
        )
        .bind(user_id)
        .bind(product_id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 查询用户关注产品的数量
    pub async fn count_by_user(executor: PgExecutor<'_>, user_id: i64) -> RepoResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM product_watchers WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }

    /// 查询用户关注的产品（含实时库存信息）
    pub async fn find_by_user_with_inventory(
        executor: PgExecutor<'_>,
        user_id: i64,
        page: u32,
        page_size: u32,
    ) -> RepoResult<PaginatedResult<WatchedProductWithInventory>> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total = Self::count_by_user(executor, user_id).await? as u64;

        let items = sqlx::query_as::<_, WatchedProductWithInventory>(
            r#"
            SELECT
                pw.product_id,
                COALESCE(p.product_code, '') AS product_code,
                COALESCE(p.pdt_name, '') AS product_name,
                COALESCE(sl.quantity, 0) AS current_quantity,
                COALESCE(pw.safety_stock_override, COALESCE(sl.safety_stock, 0)) AS effective_safety_stock,
                (COALESCE(sl.quantity, 0) < COALESCE(pw.safety_stock_override, COALESCE(sl.safety_stock, 0))) AS is_alerting
            FROM product_watchers pw
            JOIN products p ON p.product_id = pw.product_id
            LEFT JOIN (
                SELECT product_id, SUM(quantity) as quantity, MAX(safety_stock) as safety_stock
                FROM stock_ledger GROUP BY product_id
            ) sl ON sl.product_id = pw.product_id
            WHERE pw.user_id = $1
            ORDER BY pw.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(executor)
        .await?;

        Ok(PaginatedResult::new(items, total, page, page_size))
    }

    /// 查询被关注的低库存产品（Worker 用）
    pub async fn find_watched_low_stock_products(
        executor: PgExecutor<'_>,
    ) -> RepoResult<Vec<LowStockWatchedProduct>> {
        let rows = sqlx::query_as::<_, LowStockWatchedProduct>(
            r#"
            SELECT
                pw.product_id,
                p.pdt_name AS product_name,
                sl.quantity AS current_quantity,
                MIN(COALESCE(pw.safety_stock_override, sl.safety_stock)) AS effective_safety_stock
            FROM product_watchers pw
            JOIN products p ON p.product_id = pw.product_id
            JOIN (
                SELECT product_id, SUM(quantity) as quantity, MAX(safety_stock) as safety_stock
                FROM stock_ledger GROUP BY product_id
            ) sl ON sl.product_id = pw.product_id
            WHERE sl.quantity < COALESCE(pw.safety_stock_override, sl.safety_stock)
            GROUP BY pw.product_id, p.pdt_name, sl.quantity
            "#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 批量查询关注者的告警状态（Worker 用）
    pub async fn batch_get_alert_status(
        executor: PgExecutor<'_>,
        product_ids: &[i64],
    ) -> RepoResult<Vec<(i64, i64, bool)>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
            "SELECT user_id, product_id, alert_active FROM product_watchers WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 批量设置告警状态为活跃（Worker 用）
    pub async fn batch_activate_alerts(
        executor: PgExecutor<'_>,
        pairs: &[(i64, i64)],
    ) -> RepoResult<()> {
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
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 批量重置已回升产品的告警状态（Worker 用）
    pub async fn batch_clear_recovered(
        executor: PgExecutor<'_>,
        low_stock_product_ids: &[i64],
    ) -> RepoResult<u64> {
        if low_stock_product_ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query(
            r#"UPDATE product_watchers
            SET alert_active = false, updated_at = now()
            WHERE alert_active = true AND product_id != ALL($1)"#,
        )
        .bind(low_stock_product_ids)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }
}
