//! 库位数据访问层
//!
//! 提供库位的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{
    Location, LocationInventoryStats, LocationWithWarehouse, WarehouseInventoryStats,
};
use crate::repositories::Executor;

/// 库位数据仓库
pub struct LocationRepo;

impl LocationRepo {
    /// 创建新库位
    pub async fn insert(
        executor: Executor<'_>,
        warehouse_id: i64,
        location_code: &str,
        location_name: Option<&str>,
        capacity: Option<i32>,
    ) -> Result<i64> {
        let location_id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO location (warehouse_id, location_code, location_name, capacity)
            VALUES ($1, $2, $3, $4)
            RETURNING location_id
            "#,
            warehouse_id,
            location_code,
            location_name,
            capacity
        )
        .fetch_one(executor)
        .await?;

        Ok(location_id)
    }

    /// 更新库位
    pub async fn update(
        executor: Executor<'_>,
        location_id: i64,
        location_code: &str,
        location_name: Option<&str>,
        capacity: Option<i32>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE location
            SET location_code = $1, location_name = $2, capacity = $3
            WHERE location_id = $4
            "#,
            location_code,
            location_name,
            capacity,
            location_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 软删除库位
    pub async fn soft_delete(executor: Executor<'_>, location_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE location SET deleted_at = NOW() WHERE location_id = $1 AND deleted_at IS NULL",
            location_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 硬删除库位
    pub async fn hard_delete(executor: Executor<'_>, location_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM location WHERE location_id = $1", location_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 根据 ID 查找库位（排除已删除）
    pub async fn find_by_id(pool: &PgPool, location_id: i64) -> Result<Option<Location>> {
        let row = sqlx::query_as!(
            Location,
            "SELECT location_id, warehouse_id, location_code, location_name, capacity, created_at, deleted_at
             FROM location WHERE location_id = $1 AND deleted_at IS NULL",
            location_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 根据仓库和编码查找库位（排除已删除）
    pub async fn find_by_code(
        pool: &PgPool,
        warehouse_id: i64,
        location_code: &str,
    ) -> Result<Option<Location>> {
        let row = sqlx::query_as!(
            Location,
            "SELECT location_id, warehouse_id, location_code, location_name, capacity, created_at, deleted_at
             FROM location WHERE warehouse_id = $1 AND location_code = $2 AND deleted_at IS NULL",
            warehouse_id,
            location_code
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 获取仓库下所有库位（排除已删除）
    pub async fn list_by_warehouse(pool: &PgPool, warehouse_id: i64) -> Result<Vec<Location>> {
        let rows = sqlx::query_as!(
            Location,
            "SELECT location_id, warehouse_id, location_code, location_name, capacity, created_at, deleted_at
             FROM location WHERE warehouse_id = $1 AND deleted_at IS NULL
             ORDER BY location_code",
            warehouse_id
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 获取库位及仓库信息（排除已删除）
    pub async fn get_with_warehouse(
        pool: &PgPool,
        location_id: i64,
    ) -> Result<Option<LocationWithWarehouse>> {
        let row = sqlx::query_as!(
            LocationWithWarehouse,
            r#"
            SELECT l.location_id, l.location_code, l.location_name, l.capacity,
                   l.warehouse_id, w.warehouse_name, w.warehouse_code
            FROM location l
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            WHERE l.location_id = $1 AND l.deleted_at IS NULL AND w.deleted_at IS NULL
            "#,
            location_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 检查库位编码在仓库下是否已存在（排除已删除）
    pub async fn code_exists_in_warehouse(
        pool: &PgPool,
        warehouse_id: i64,
        location_code: &str,
    ) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM location WHERE warehouse_id = $1 AND location_code = $2 AND deleted_at IS NULL",
        )
        .bind(warehouse_id)
        .bind(location_code)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// 检查库位下是否有库存
    pub async fn has_inventory(pool: &PgPool, location_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM inventory WHERE location_id = $1",
        )
        .bind(location_id)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    // ========================================================================
    // 库存统计查询
    // ========================================================================

    /// 获取仓库库存统计汇总
    pub async fn get_warehouse_inventory_stats(
        pool: &PgPool,
        warehouse_id: i64,
    ) -> Result<Option<WarehouseInventoryStats>> {
        let stats = sqlx::query_as::<_, WarehouseInventoryStats>(
            r#"
            SELECT
                w.warehouse_id,
                w.warehouse_name,
                w.warehouse_code,
                COALESCE(SUM(i.quantity), 0)::bigint as total_quantity,
                COUNT(DISTINCT l.location_id)::bigint as location_count,
                COUNT(DISTINCT i.product_id)::bigint as product_count,
                COUNT(DISTINCT CASE WHEN i.quantity < i.safety_stock THEN i.product_id END)::bigint as low_stock_count
            FROM warehouse w
            LEFT JOIN location l ON w.warehouse_id = l.warehouse_id
            LEFT JOIN inventory i ON l.location_id = i.location_id
            WHERE w.warehouse_id = $1
            GROUP BY w.warehouse_id, w.warehouse_name, w.warehouse_code
            "#,
        )
        .bind(warehouse_id)
        .fetch_optional(pool)
        .await?;

        Ok(stats)
    }

    /// 获取库位库存统计
    pub async fn get_location_inventory_stats(
        pool: &PgPool,
        location_id: i64,
    ) -> Result<Option<LocationInventoryStats>> {
        let stats = sqlx::query_as::<_, LocationInventoryStats>(
            r#"
            SELECT
                l.location_id,
                l.location_code,
                l.location_name,
                COALESCE(SUM(i.quantity), 0)::bigint as total_quantity,
                COUNT(DISTINCT i.product_id)::bigint as product_count,
                COUNT(DISTINCT CASE WHEN i.quantity < i.safety_stock THEN i.product_id END)::bigint as low_stock_count
            FROM location l
            LEFT JOIN inventory i ON l.location_id = i.location_id
            WHERE l.location_id = $1
            GROUP BY l.location_id, l.location_code, l.location_name
            "#,
        )
        .bind(location_id)
        .fetch_optional(pool)
        .await?;

        Ok(stats)
    }

    /// 分页获取仓库下所有库位的库存统计
    pub async fn list_location_stats_by_warehouse(
        pool: &PgPool,
        warehouse_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<LocationInventoryStats>, u64)> {
        let offset = (page - 1) * page_size;

        // 查询总数
        let total: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM location WHERE warehouse_id = $1",
        )
        .bind(warehouse_id)
        .fetch_one(pool)
        .await?;

        // 查询数据
        let items = sqlx::query_as::<_, LocationInventoryStats>(
            r#"
            SELECT
                l.location_id,
                l.location_code,
                l.location_name,
                COALESCE(SUM(i.quantity), 0)::bigint as total_quantity,
                COUNT(DISTINCT i.product_id)::bigint as product_count,
                COUNT(DISTINCT CASE WHEN i.quantity < i.safety_stock THEN i.product_id END)::bigint as low_stock_count
            FROM location l
            LEFT JOIN inventory i ON l.location_id = i.location_id
            WHERE l.warehouse_id = $1
            GROUP BY l.location_id, l.location_code, l.location_name
            ORDER BY l.location_code
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(warehouse_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        Ok((items, total as u64))
    }

    // ========================================================================
    // Excel 导入辅助方法
    // ========================================================================

    /// 根据仓库名称和库位编码查找库位
    pub async fn find_by_warehouse_name_and_code(
        pool: &PgPool,
        warehouse_name: &str,
        location_code: &str,
    ) -> Result<Option<Location>> {
        let row = sqlx::query_as!(
            Location,
            "SELECT l.location_id, l.warehouse_id, l.location_code, l.location_name, l.capacity, l.created_at, l.deleted_at
             FROM location l
             JOIN warehouse w ON l.warehouse_id = w.warehouse_id
             WHERE w.warehouse_name = $1 AND l.location_code = $2 AND l.deleted_at IS NULL AND w.deleted_at IS NULL",
            warehouse_name,
            location_code
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 根据仓库名称获取默认库位（第一个库位）
    pub async fn find_default_by_warehouse_name(
        pool: &PgPool,
        warehouse_name: &str,
    ) -> Result<Option<Location>> {
        let row = sqlx::query_as!(
            Location,
            "SELECT l.location_id, l.warehouse_id, l.location_code, l.location_name, l.capacity, l.created_at, l.deleted_at
             FROM location l
             JOIN warehouse w ON l.warehouse_id = w.warehouse_id
             WHERE w.warehouse_name = $1 AND l.deleted_at IS NULL AND w.deleted_at IS NULL
             ORDER BY l.location_id LIMIT 1",
            warehouse_name
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 批量获取所有库位（带仓库名称），用于 Excel 导入
    /// 返回 HashMap: (warehouse_name, location_code) -> Location
    pub async fn list_all_with_warehouse(pool: &PgPool) -> Result<std::collections::HashMap<(String, String), Location>> {
        let rows = sqlx::query_as!(
            Location,
            "SELECT l.location_id, l.warehouse_id, l.location_code, l.location_name, l.capacity, l.created_at, l.deleted_at
             FROM location l
             JOIN warehouse w ON l.warehouse_id = w.warehouse_id
             WHERE l.deleted_at IS NULL AND w.deleted_at IS NULL"
        )
        .fetch_all(pool)
        .await?;

        // 获取仓库名称映射
        let warehouse_ids: Vec<i64> = rows.iter().map(|l| l.warehouse_id).collect();
        let warehouses = sqlx::query!(
            "SELECT warehouse_id, warehouse_name FROM warehouse WHERE warehouse_id = ANY($1) AND deleted_at IS NULL",
            &warehouse_ids
        )
        .fetch_all(pool)
        .await?;

        let warehouse_map: std::collections::HashMap<i64, String> = warehouses
            .into_iter()
            .map(|w| (w.warehouse_id, w.warehouse_name))
            .collect();

        let mut map = std::collections::HashMap::new();
        for location in rows {
            if let Some(warehouse_name) = warehouse_map.get(&location.warehouse_id) {
                map.insert(
                    (warehouse_name.clone(), location.location_code.clone()),
                    location,
                );
            }
        }

        Ok(map)
    }
}
