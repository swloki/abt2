//! 工艺路线数据访问层
//!
//! 提供 routing、routing_step、bom_routing 表的 CRUD 操作。

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::models::{BomRouting, Routing, RoutingStep};
use crate::repositories::Executor;

/// 路线引用的 BOM 简要信息
#[derive(Debug, Clone, FromRow)]
pub struct BomBrief {
    pub bom_id: i64,
    pub bom_name: String,
    pub created_at: DateTime<Utc>,
}

/// 工艺路线仓库
pub struct RoutingRepo;

impl RoutingRepo {
    // ========================================================================
    // routing 表查询
    // ========================================================================

    /// 分页查询工艺路线（支持按名称模糊搜索）
    pub async fn find_all(
        pool: &PgPool,
        keyword: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<Routing>> {
        let offset = (page.max(1) - 1) * page_size.clamp(1, 100);
        let items: Vec<Routing> = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_as(
                "SELECT id, name, description, created_at, updated_at \
                 FROM routing \
                 WHERE name ILIKE $1 \
                 ORDER BY id ASC \
                 LIMIT $2 OFFSET $3",
            )
            .bind(&pattern)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, description, created_at, updated_at \
                 FROM routing \
                 ORDER BY id ASC \
                 LIMIT $1 OFFSET $2",
            )
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    /// 统计工艺路线数量（支持按名称模糊搜索）
    pub async fn count_all(pool: &PgPool, keyword: Option<&str>) -> Result<i64> {
        let count: i64 = if let Some(kw) = keyword {
            let pattern = format!("%{kw}%");
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM routing WHERE name ILIKE $1",
            )
            .bind(&pattern)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM routing")
                .fetch_one(pool)
                .await?
        };
        Ok(count)
    }

    /// 按 ID 查询单条路线
    pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Routing>> {
        let item = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at \
             FROM routing WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// 按 ID 查询单条路线（使用 Executor，用于事务内调用）
    pub async fn find_by_id_tx(executor: Executor<'_>, id: i64) -> Result<Option<Routing>> {
        let item = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at \
             FROM routing WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(item)
    }

    // ========================================================================
    // routing_step 表查询
    // ========================================================================

    /// 查询路线的所有工序
    pub async fn find_steps_by_routing_id(pool: &PgPool, routing_id: i64) -> Result<Vec<RoutingStep>> {
        let steps: Vec<RoutingStep> = sqlx::query_as(
            "SELECT id, routing_id, process_code, step_order, is_required, remark, created_at, updated_at \
             FROM routing_step \
             WHERE routing_id = $1 \
             ORDER BY step_order ASC, id ASC",
        )
        .bind(routing_id)
        .fetch_all(pool)
        .await?;
        Ok(steps)
    }

    /// 查询路线的所有工序（使用 Executor，用于事务内调用）
    pub async fn find_steps_by_routing_id_tx(executor: Executor<'_>, routing_id: i64) -> Result<Vec<RoutingStep>> {
        let steps: Vec<RoutingStep> = sqlx::query_as(
            "SELECT id, routing_id, process_code, step_order, is_required, remark, created_at, updated_at \
             FROM routing_step \
             WHERE routing_id = $1 \
             ORDER BY step_order ASC, id ASC",
        )
        .bind(routing_id)
        .fetch_all(executor)
        .await?;
        Ok(steps)
    }

    // ========================================================================
    // routing 表写入
    // ========================================================================

    /// 创建工艺路线，返回 ID
    pub async fn insert_routing(executor: Executor<'_>, name: &str, description: Option<&str>) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO routing (name, description)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(description)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 更新工艺路线
    pub async fn update_routing(
        executor: Executor<'_>,
        id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE routing
            SET name = $1, description = $2, updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(id)
        .execute(executor)
        .await?;
        if result.rows_affected() == 0 {
            return Err(common::error::ServiceError::NotFound {
                resource: "工艺路线".to_string(),
                id: id.to_string(),
            }.into());
        }
        Ok(())
    }

    /// 删除工艺路线
    pub async fn delete_routing(executor: Executor<'_>, id: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM routing WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // routing_step 表写入
    // ========================================================================

    /// 批量插入路线工序
    pub async fn batch_insert_steps(
        executor: Executor<'_>,
        routing_id: i64,
        steps: &[crate::models::RoutingStepInput],
    ) -> Result<()> {
        if steps.is_empty() {
            return Ok(());
        }
        let mut builder = sqlx::QueryBuilder::new(
            "INSERT INTO routing_step (routing_id, process_code, step_order, is_required, remark) "
        );
        builder.push_values(steps.iter(), |mut b, step| {
            b.push_bind(routing_id)
                .push_bind(&step.process_code)
                .push_bind(step.step_order)
                .push_bind(step.is_required)
                .push_bind(&step.remark);
        });
        builder.build().execute(executor).await?;
        Ok(())
    }

    /// 删除路线的所有工序
    pub async fn delete_steps_by_routing_id(executor: Executor<'_>, routing_id: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM routing_step WHERE routing_id = $1",
        )
        .bind(routing_id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    // ========================================================================
    // 匹配查询：精确集合匹配
    // ========================================================================

    /// 查找与给定工序编码集合完全匹配的路线 ID
    ///
    /// 使用 SQL 精确集合匹配：路线的所有工序编码必须恰好等于输入集合，
    /// 不多不少。返回第一个匹配的 routing_id。
    /// JOIN routing 表确保只返回未被删除的路线。
    pub async fn find_matching_routing(pool: &PgPool, process_codes: &[String]) -> Result<Option<i64>> {
        if process_codes.is_empty() {
            return Ok(None);
        }

        let routing_id: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT rs.routing_id
            FROM routing_step rs
            JOIN routing r ON r.id = rs.routing_id
            GROUP BY rs.routing_id
            HAVING array_agg(DISTINCT rs.process_code ORDER BY rs.process_code) = (
                SELECT array_agg(DISTINCT v ORDER BY v)
                FROM unnest($1::varchar[]) AS v
            )
            AND COUNT(DISTINCT rs.process_code) = $2
            LIMIT 1
            "#,
        )
        .bind(process_codes)
        .bind(process_codes.len() as i64)
        .fetch_optional(pool)
        .await?;
        Ok(routing_id)
    }

    /// 同上，但接受 Executor（用于事务内调用）
    pub async fn find_matching_routing_tx(executor: Executor<'_>, process_codes: &[String]) -> Result<Option<i64>> {
        if process_codes.is_empty() {
            return Ok(None);
        }

        let routing_id: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT rs.routing_id
            FROM routing_step rs
            JOIN routing r ON r.id = rs.routing_id
            GROUP BY rs.routing_id
            HAVING array_agg(DISTINCT rs.process_code ORDER BY rs.process_code) = (
                SELECT array_agg(DISTINCT v ORDER BY v)
                FROM unnest($1::varchar[]) AS v
            )
            AND COUNT(DISTINCT rs.process_code) = $2
            LIMIT 1
            "#,
        )
        .bind(process_codes)
        .bind(process_codes.len() as i64)
        .fetch_optional(executor)
        .await?;
        Ok(routing_id)
    }

    // ========================================================================
    // bom_routing 表操作
    // ========================================================================

    /// 查询 BOM 路线绑定
    pub async fn find_bom_routing(pool: &PgPool, product_code: &str) -> Result<Option<BomRouting>> {
        let item = sqlx::query_as(
            "SELECT id, product_code, routing_id, created_at, updated_at \
             FROM bom_routing WHERE product_code = $1",
        )
        .bind(product_code)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// 查询 BOM 路线绑定（使用 Executor，用于事务内调用）
    pub async fn find_bom_routing_tx(executor: Executor<'_>, product_code: &str) -> Result<Option<BomRouting>> {
        let item = sqlx::query_as(
            "SELECT id, product_code, routing_id, created_at, updated_at \
             FROM bom_routing WHERE product_code = $1",
        )
        .bind(product_code)
        .fetch_optional(executor)
        .await?;
        Ok(item)
    }

    /// 设置 BOM 路线绑定（upsert）
    pub async fn set_bom_routing(
        executor: Executor<'_>,
        product_code: &str,
        routing_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO bom_routing (product_code, routing_id)
            VALUES ($1, $2)
            ON CONFLICT (product_code)
            DO UPDATE SET routing_id = $2, updated_at = NOW()
            "#,
        )
        .bind(product_code)
        .bind(routing_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 删除 BOM 路线绑定
    pub async fn delete_bom_routing(executor: Executor<'_>, product_code: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM bom_routing WHERE product_code = $1",
        )
        .bind(product_code)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 检查路线是否被任何产品绑定
    pub async fn exists_bom_routing_by_routing_id(pool: &PgPool, routing_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bom_routing WHERE routing_id = $1",
        )
        .bind(routing_id)
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
    }

    /// 查询引用指定路线的 BOM 列表（分页）
    pub async fn find_boms_by_routing_id(
        pool: &PgPool,
        routing_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<BomBrief>, i64)> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT b.bom_id)
            FROM bom_routing br
            JOIN products p ON p.meta->>'product_code' = br.product_code
            JOIN bom_nodes bn ON bn.product_id = p.product_id AND bn.parent_id IS NULL
            JOIN bom b ON b.bom_id = bn.bom_id
            WHERE br.routing_id = $1
            "#,
        )
        .bind(routing_id)
        .fetch_one(pool)
        .await?;

        let items: Vec<BomBrief> = sqlx::query_as(
            r#"
            SELECT DISTINCT b.bom_id, b.bom_name, b.create_at as created_at
            FROM bom_routing br
            JOIN products p ON p.meta->>'product_code' = br.product_code
            JOIN bom_nodes bn ON bn.product_id = p.product_id AND bn.parent_id IS NULL
            JOIN bom b ON b.bom_id = bn.bom_id
            WHERE br.routing_id = $1
            ORDER BY b.bom_id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(routing_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        Ok((items, total))
    }
}
