//! 工序字典数据访问层
//!
//! 提供 labor_process_dict 表的 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::LaborProcessDict;
use crate::repositories::Executor;

/// 工序字典仓库
pub struct LaborProcessDictRepo;

impl LaborProcessDictRepo {
    // ========================================================================
    // 查询
    // ========================================================================

    /// 分页查询工序字典（支持按编码或名称模糊搜索）
    pub async fn find_all(
        pool: &PgPool,
        keyword: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<LaborProcessDict>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);
        let items: Vec<LaborProcessDict> = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_as(
                "SELECT id, code, name, description, sort_order, created_at, updated_at \
                 FROM labor_process_dict \
                 WHERE code ILIKE $1 OR name ILIKE $1 \
                 ORDER BY sort_order ASC, id ASC \
                 LIMIT $2 OFFSET $3"
            )
            .bind(&pattern)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, code, name, description, sort_order, created_at, updated_at \
                 FROM labor_process_dict \
                 ORDER BY sort_order ASC, id ASC \
                 LIMIT $1 OFFSET $2"
            )
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    /// 统计工序字典数量（支持按编码或名称模糊搜索）
    pub async fn count_all(pool: &PgPool, keyword: Option<&str>) -> Result<i64> {
        let count: i64 = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM labor_process_dict WHERE code ILIKE $1 OR name ILIKE $1"
            )
            .bind(&pattern)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM labor_process_dict"
            )
            .fetch_one(pool)
            .await?
        };
        Ok(count)
    }

    /// 按 ID 查询单条记录
    pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<LaborProcessDict>> {
        let item = sqlx::query_as(
            "SELECT id, code, name, description, sort_order, created_at, updated_at \
             FROM labor_process_dict WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// 按编码查询
    pub async fn find_by_code(pool: &PgPool, code: &str) -> Result<Option<LaborProcessDict>> {
        let item = sqlx::query_as(
            "SELECT id, code, name, description, sort_order, created_at, updated_at \
             FROM labor_process_dict WHERE code = $1"
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// 查询所有字典记录（用于导出，按 sort_order 排序）
    pub async fn list_all(pool: &PgPool) -> Result<Vec<LaborProcessDict>> {
        let items: Vec<LaborProcessDict> = sqlx::query_as(
            "SELECT id, code, name, description, sort_order, created_at, updated_at \
             FROM labor_process_dict \
             ORDER BY sort_order ASC, id ASC"
        )
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    // ========================================================================
    // 写入
    // ========================================================================

    /// 创建工序字典
    pub async fn insert(
        executor: Executor<'_>,
        code: &str,
        name: &str,
        description: Option<&str>,
        sort_order: i32,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO labor_process_dict (code, name, description, sort_order)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#
        )
        .bind(code)
        .bind(name)
        .bind(description)
        .bind(sort_order)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 更新工序字典（code 不可修改）
    pub async fn update(
        executor: Executor<'_>,
        id: i64,
        name: &str,
        description: Option<&str>,
        sort_order: i32,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE labor_process_dict
            SET name = $1, description = $2, sort_order = $3, updated_at = NOW()
            WHERE id = $4
            "#
        )
        .bind(name)
        .bind(description)
        .bind(sort_order)
        .bind(id)
        .execute(executor)
        .await?;
        if result.rows_affected() == 0 {
            return Err(common::error::ServiceError::NotFound {
                resource: "工序字典".to_string(),
                id: id.to_string(),
            }.into());
        }
        Ok(())
    }

    /// 删除工序字典
    pub async fn delete(executor: Executor<'_>, id: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM labor_process_dict WHERE id = $1"
        )
        .bind(id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // 批量校验
    // ========================================================================

    /// 查询给定编码中存在于字典中的编码集合
    pub async fn find_existing_codes(pool: &PgPool, codes: &[String]) -> Result<std::collections::HashSet<String>> {
        if codes.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT code FROM labor_process_dict WHERE code = ANY($1)"
        )
        .bind(codes)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    // ========================================================================
    // 关联检查
    // ========================================================================

    /// 检查工序编码是否被 routing_step 引用
    pub async fn exists_by_process_code(pool: &PgPool, code: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM routing_step WHERE process_code = $1"
        )
        .bind(code)
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
    }
}
