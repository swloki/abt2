//! 仓库数据访问层
//!
//! 提供仓库的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::Warehouse;
use crate::repositories::Executor;

/// 仓库数据仓库
pub struct WarehouseRepo;

impl WarehouseRepo {
    /// 创建新仓库
    pub async fn insert(
        executor: Executor<'_>,
        warehouse_name: &str,
        warehouse_code: &str,
    ) -> Result<i64> {
        let warehouse_id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO warehouse (warehouse_name, warehouse_code, status)
            VALUES ($1, $2, 'active')
            RETURNING warehouse_id
            "#,
            warehouse_name,
            warehouse_code
        )
        .fetch_one(executor)
        .await?;

        Ok(warehouse_id)
    }

    /// 更新仓库
    pub async fn update(
        executor: Executor<'_>,
        warehouse_id: i64,
        warehouse_name: &str,
        warehouse_code: Option<&str>,
        status: &str,
    ) -> Result<()> {
        if let Some(code) = warehouse_code {
            sqlx::query!(
                r#"
                UPDATE warehouse
                SET warehouse_name = $1, warehouse_code = $2, status = $3, updated_at = NOW()
                WHERE warehouse_id = $4
                "#,
                warehouse_name,
                code,
                status,
                warehouse_id
            )
            .execute(executor)
            .await?;
        } else {
            sqlx::query!(
                r#"
                UPDATE warehouse
                SET warehouse_name = $1, status = $2, updated_at = NOW()
                WHERE warehouse_id = $3
                "#,
                warehouse_name,
                status,
                warehouse_id
            )
            .execute(executor)
            .await?;
        }

        Ok(())
    }

    /// 软删除仓库
    pub async fn soft_delete(executor: Executor<'_>, warehouse_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE warehouse SET deleted_at = NOW() WHERE warehouse_id = $1 AND deleted_at IS NULL",
            warehouse_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 硬删除仓库
    pub async fn hard_delete(executor: Executor<'_>, warehouse_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM warehouse WHERE warehouse_id = $1", warehouse_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 根据 ID 查找仓库（排除已删除）
    pub async fn find_by_id(pool: &PgPool, warehouse_id: i64) -> Result<Option<Warehouse>> {
        let row = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE warehouse_id = $1 AND deleted_at IS NULL",
        )
        .bind(warehouse_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 根据编码查找仓库（排除已删除）
    pub async fn find_by_code(pool: &PgPool, warehouse_code: &str) -> Result<Option<Warehouse>> {
        let row = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE warehouse_code = $1 AND deleted_at IS NULL",
        )
        .bind(warehouse_code)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 获取所有活跃仓库（排除已删除）
    pub async fn list_active(pool: &PgPool) -> Result<Vec<Warehouse>> {
        let rows = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE status = 'active' AND deleted_at IS NULL
             ORDER BY warehouse_id",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 获取所有仓库（排除已删除）
    pub async fn list_all(pool: &PgPool) -> Result<Vec<Warehouse>> {
        let rows = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE deleted_at IS NULL ORDER BY warehouse_id",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 检查仓库编码是否已存在（排除已删除）
    pub async fn code_exists(pool: &PgPool, warehouse_code: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM warehouse WHERE warehouse_code = $1 AND deleted_at IS NULL",
        )
        .bind(warehouse_code)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// 检查仓库下是否有库位
    pub async fn has_locations(pool: &PgPool, warehouse_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM location WHERE warehouse_id = $1 AND deleted_at IS NULL",
        )
        .bind(warehouse_id)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// 批量根据编码查找仓库（排除已删除）
    pub async fn find_by_codes(pool: &PgPool, codes: &[String]) -> Result<Vec<Warehouse>> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE warehouse_code = ANY($1) AND deleted_at IS NULL",
        )
        .bind(codes)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 批量根据编码查找已删除的仓库
    pub async fn find_deleted_by_codes(pool: &PgPool, codes: &[String]) -> Result<Vec<Warehouse>> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE warehouse_code = ANY($1) AND deleted_at IS NOT NULL",
        )
        .bind(codes)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 根据编码查找已删除的仓库
    pub async fn find_deleted_by_code(pool: &PgPool, code: &str) -> Result<Option<Warehouse>> {
        let row = sqlx::query_as::<_, Warehouse>(
            "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at
             FROM warehouse WHERE warehouse_code = $1 AND deleted_at IS NOT NULL",
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 检查仓库下是否有库存
    pub async fn has_inventory(pool: &PgPool, warehouse_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM inventory i
            JOIN location l ON i.location_id = l.location_id
            WHERE l.warehouse_id = $1
            "#,
        )
        .bind(warehouse_id)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }
}
