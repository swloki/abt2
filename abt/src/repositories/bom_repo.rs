//! BOM 数据访问层
//!
//! 提供 BOM 的数据库 CRUD 操作。
//! 节点数据从 bom_nodes 表读写，不再操作 bom_detail JSONB。

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgPool;

use crate::models::BomQuery;
use crate::repositories::{build_fuzzy_pattern, Executor};

/// BOM 简要信息（用于引用显示）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomReference {
    pub bom_id: i64,
    pub bom_name: String,
}

/// 产品使用情况（包含 BOM 列表和总数）
#[derive(Debug, Clone)]
pub struct ProductUsageResult {
    pub boms: Vec<BomReference>,
    pub total: i64,
}

/// BOM 数据仓库
pub struct BomRepo;

impl BomRepo {
    /// 创建新的 BOM
    pub async fn insert(
        executor: Executor<'_>,
        bom_name: &str,
        bom_category_id: Option<i64>,
        created_by: Option<i64>,
        status: &str,
    ) -> Result<i64> {
        let bom_id: i64 = sqlx::query_scalar(
            "INSERT INTO bom (bom_name, create_at, bom_category_id, created_by, status) VALUES ($1, NOW(), $2, $3, $4) RETURNING bom_id",
        )
        .bind(bom_name)
        .bind(bom_category_id)
        .bind(created_by)
        .bind(status)
        .fetch_one(executor)
        .await?;

        Ok(bom_id)
    }

    /// 更新 BOM 状态（发布/取消发布）
    pub async fn update_status(
        executor: Executor<'_>,
        bom_id: i64,
        status: &str,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<crate::models::Bom> {
        let bom = sqlx::query_as::<_, crate::models::Bom>(
            "UPDATE bom SET status = $1, published_at = $2, update_at = NOW() WHERE bom_id = $3 RETURNING bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at",
        )
        .bind(status)
        .bind(published_at)
        .bind(bom_id)
        .fetch_one(executor)
        .await?;

        Ok(bom)
    }

    /// 更新 BOM 元数据（名称和分类）
    pub async fn update(
        executor: Executor<'_>,
        bom_id: i64,
        bom_name: &str,
        bom_category_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE bom SET bom_name = $1, bom_category_id = $2, update_at = NOW() WHERE bom_id = $3",
        )
        .bind(bom_name)
        .bind(bom_category_id)
        .bind(bom_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 删除 BOM
    pub async fn delete(executor: Executor<'_>, bom_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM bom WHERE bom_id = $1")
            .bind(bom_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 根据 ID 查找 BOM
    pub async fn find_by_id(executor: Executor<'_>, bom_id: i64) -> Result<Option<crate::models::Bom>> {
        let row = sqlx::query_as::<_, crate::models::Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at FROM bom WHERE bom_id = $1",
        )
        .bind(bom_id)
        .fetch_optional(executor)
        .await?;

        Ok(row)
    }

    /// 查找 BOM 并加行锁（用于先读后写模式，必须在事务内调用）
    pub async fn find_by_id_for_update(executor: Executor<'_>, bom_id: i64) -> Result<Option<crate::models::Bom>> {
        let row = sqlx::query_as::<_, crate::models::Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at FROM bom WHERE bom_id = $1 FOR UPDATE",
        )
        .bind(bom_id)
        .fetch_optional(executor)
        .await?;

        Ok(row)
    }

    /// 批量查询包含指定产品且用户有权访问的 BOM（带行锁）
    pub async fn find_accessible_boms_by_product(
        executor: Executor<'_>,
        product_id: i64,
        caller_id: i64,
    ) -> Result<Vec<crate::models::Bom>> {
        let rows = sqlx::query_as::<_, crate::models::Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at
             FROM bom
             WHERE EXISTS (SELECT 1 FROM bom_nodes WHERE bom_nodes.bom_id = bom.bom_id AND product_id = $1)
               AND (status = 'published' OR created_by = $2)
             FOR UPDATE",
        )
        .bind(product_id)
        .bind(caller_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 使用连接池查找 BOM（用于只读操作）
    pub async fn find_by_id_pool(pool: &PgPool, bom_id: i64) -> Result<Option<crate::models::Bom>> {
        let row = sqlx::query_as::<_, crate::models::Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at FROM bom WHERE bom_id = $1",
        )
        .bind(bom_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 检查 BOM 名称是否存在（限定已发布 BOM + 当前用户的草稿）
    pub async fn exists_name(pool: &PgPool, name: &str, caller_id: Option<i64>) -> Result<bool> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bom WHERE bom_name = $1 AND (status = 'published' OR created_by = $2))",
        )
        .bind(name)
        .bind(caller_id)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    /// 查询 BOM 列表
    pub async fn query(pool: &PgPool, bom_query: &BomQuery) -> Result<Vec<crate::models::Bom>> {
        let mut query = sqlx::QueryBuilder::new(
            r#"
            SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at
            FROM bom
            WHERE 1=1
            "#,
        );

        Self::build_query_filter(&mut query, bom_query);

        let page = bom_query.page.unwrap_or(1).max(1);
        let page_size = bom_query.page_size.unwrap_or(12).clamp(1, 100);

        query.push(" ORDER BY bom_id DESC");
        query.push(" LIMIT ");
        query.push_bind(page_size as i32);
        query.push(" OFFSET ");
        query.push_bind(((page - 1) * page_size) as i32);

        let result = query.build_query_as::<crate::models::Bom>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询 BOM 总数
    pub async fn query_count(pool: &PgPool, bom_query: &BomQuery) -> Result<i64> {
        let mut query = sqlx::QueryBuilder::new("SELECT count(*) FROM bom WHERE 1=1");

        Self::build_query_filter(&mut query, bom_query);

        let count: i64 = query.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 构建查询过滤条件
    fn build_query_filter(
        query: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
        bom_query: &BomQuery,
    ) {
        if let Some(bom_name) = &bom_query.bom_name
            && !bom_name.is_empty()
            && let Some(pattern) = build_fuzzy_pattern(bom_name) {
                query.push(" AND bom_name ILIKE ");
                query.push_bind(pattern);
            }
        if let Some(create_by) = &bom_query.create_by
            && !create_by.is_empty()
        {
            query.push(" AND created_by::text ILIKE ");
            query.push_bind(format!("%{}%", create_by));
        }
        if let Some(date_from) = &bom_query.date_from
            && let Ok(date) = NaiveDate::parse_from_str(date_from, "%Y-%m-%d")
        {
            let naive_dt = date.and_hms_opt(0, 0, 0).unwrap_or_default();
            let utc_datetime = naive_dt.and_utc();
            query.push(" AND create_at >= ");
            query.push_bind(utc_datetime);
        }
        if let Some(date_to) = &bom_query.date_to
            && let Ok(date) = NaiveDate::parse_from_str(date_to, "%Y-%m-%d")
        {
            let naive_dt = date.and_hms_opt(23, 59, 59).unwrap_or_default();
            let utc_datetime = naive_dt.and_utc();
            query.push(" AND create_at <= ");
            query.push_bind(utc_datetime);
        }
        if let Some(product_id) = bom_query.product_id {
            query.push(" AND EXISTS (SELECT 1 FROM bom_nodes WHERE bom_nodes.bom_id = bom.bom_id AND product_id = ");
            query.push_bind(product_id);
            query.push(")");
        }
        if let Some(product_code) = &bom_query.product_code
            && !product_code.is_empty()
        {
            query.push(" AND EXISTS (SELECT 1 FROM bom_nodes n JOIN products p ON n.product_id = p.product_id WHERE n.bom_id = bom.bom_id AND n.parent_id IS NULL AND p.meta->>'product_code' ILIKE ");
            query.push_bind(format!("%{}%", product_code));
            query.push(")");
        }
        if let Some(bom_category_id) = bom_query.bom_category_id {
            query.push(" AND bom_category_id = ");
            query.push_bind(bom_category_id);
        }
        if let Some(status) = &bom_query.status {
            query.push(" AND status = ");
            query.push_bind(status.as_str());
        }
        if let Some(caller_id) = bom_query.caller_id {
            query.push(" AND (status = 'published' OR created_by = ");
            query.push_bind(caller_id);
            query.push(")");
        }
    }

    /// 查询使用指定产品的 BOM 列表
    pub async fn find_boms_using_product(
        pool: &PgPool,
        product_id: i64,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<ProductUsageResult> {
        let ps = page_size.unwrap_or(10) as i64;
        let offset = (page.unwrap_or(1).saturating_sub(1)) as i64 * ps;

        let total: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bom WHERE status = 'published' AND EXISTS (SELECT 1 FROM bom_nodes WHERE bom_nodes.bom_id = bom.bom_id AND product_id = $1)",
        )
        .bind(product_id)
        .fetch_one(pool)
        .await?;
        let total = total.unwrap_or(0);

        let boms: Vec<BomReference> = sqlx::query_as(
            r#"
            SELECT bom_id, bom_name
            FROM bom
            WHERE status = 'published'
            AND EXISTS (
                SELECT 1 FROM bom_nodes WHERE bom_nodes.bom_id = bom.bom_id AND product_id = $1
            )
            ORDER BY bom_name
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(product_id)
        .bind(ps)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(ProductUsageResult { boms, total })
    }

    /// 查询所有包含指定产品的完整 BOM 列表（用于物料替换）
    pub async fn find_all_boms_using_product(
        executor: Executor<'_>,
        product_id: i64,
    ) -> Result<Vec<crate::models::Bom>> {
        let rows = sqlx::query_as::<_, crate::models::Bom>(
            r#"
            SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at
            FROM bom
            WHERE EXISTS (
                SELECT 1 FROM bom_nodes WHERE bom_nodes.bom_id = bom.bom_id AND product_id = $1
            )
            FOR UPDATE
            "#,
        )
        .bind(product_id)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    /// 批量查询哪些 product_code 有对应的 BOM（根节点）
    pub async fn find_product_codes_with_bom(
        pool: &PgPool,
        product_codes: &[String],
    ) -> Result<Vec<String>> {
        if product_codes.is_empty() {
            return Ok(Vec::new());
        }

        let codes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT p.meta->>'product_code'
            FROM products p
            JOIN bom_nodes bn ON bn.product_id = p.product_id AND bn.parent_id IS NULL
            JOIN bom b ON b.bom_id = bn.bom_id
            WHERE p.meta->>'product_code' = ANY($1)
            "#,
        )
        .bind(product_codes)
        .fetch_all(pool)
        .await?;

        Ok(codes)
    }
}
