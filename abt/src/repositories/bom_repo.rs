//! BOM 数据访问层
//!
//! 提供 BOM 的数据库 CRUD 操作。

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;

use crate::models::{Bom, BomDetail, BomQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

/// BOM 简要信息（用于引用显示）
#[derive(Debug, Clone)]
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
        bom_detail: &BomDetail,
        bom_category_id: Option<i64>,
    ) -> Result<i64> {
        let bom_id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO bom (bom_name, create_at, bom_detail, bom_category_id)
            VALUES ($1, NOW(), $2::jsonb, $3)
            RETURNING bom_id
            "#,
            bom_name,
            json!(bom_detail),
            bom_category_id
        )
        .fetch_one(executor)
        .await?;

        Ok(bom_id)
    }

    /// 更新 BOM
    pub async fn update(
        executor: Executor<'_>,
        bom_id: i64,
        bom_name: &str,
        bom_detail: Option<&BomDetail>,
        bom_category_id: Option<i64>,
    ) -> Result<()> {
        if let Some(detail) = bom_detail {
            sqlx::query!(
                r#"
                UPDATE bom
                SET bom_name = $1, bom_detail = $2::jsonb, bom_category_id = $3, update_at = NOW()
                WHERE bom_id = $4
                "#,
                bom_name,
                json!(detail),
                bom_category_id,
                bom_id
            )
            .execute(executor)
            .await?;
        } else {
            sqlx::query!(
                r#"
                UPDATE bom
                SET bom_name = $1, bom_category_id = $2, update_at = NOW()
                WHERE bom_id = $3
                "#,
                bom_name,
                bom_category_id,
                bom_id
            )
            .execute(executor)
            .await?;
        }

        Ok(())
    }

    /// 删除 BOM
    pub async fn delete(executor: Executor<'_>, bom_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM bom WHERE bom_id = $1", bom_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 根据 ID 查找 BOM
    /// 注意：Bom 有自定义 FromRow impl，需要用 runtime query
    pub async fn find_by_id(executor: Executor<'_>, bom_id: i64) -> Result<Option<Bom>> {
        let row = sqlx::query_as::<_, Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_detail::text, bom_category_id FROM bom WHERE bom_id = $1",
        )
        .bind(bom_id)
        .fetch_optional(executor)
        .await?;

        Ok(row)
    }

    /// 使用连接池查找 BOM（用于只读操作）
    /// 注意：Bom 有自定义 FromRow impl，需要用 runtime query
    #[allow(dead_code)]
    pub async fn find_by_id_pool(pool: &PgPool, bom_id: i64) -> Result<Option<Bom>> {
        let row = sqlx::query_as::<_, Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_detail::text, bom_category_id FROM bom WHERE bom_id = $1",
        )
        .bind(bom_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 检查 BOM 名称是否存在
    #[allow(dead_code)]
    pub async fn exists_name(pool: &PgPool, name: &str) -> Result<bool> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bom WHERE bom_name = $1)",
        )
        .bind(name)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    /// 查询 BOM 列表
    #[allow(dead_code)]
    pub async fn query(pool: &PgPool, bom_query: &BomQuery) -> Result<Vec<Bom>> {
        let mut query = sqlx::QueryBuilder::new(
            r#"
            SELECT bom_id, bom_name, create_at, update_at, bom_detail::text, bom_category_id
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

        let result = query.build_query_as::<Bom>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询 BOM 总数
    #[allow(dead_code)]
    pub async fn query_count(pool: &PgPool, bom_query: &BomQuery) -> Result<i64> {
        let mut query = sqlx::QueryBuilder::new("SELECT count(*) FROM bom WHERE 1=1");

        Self::build_query_filter(&mut query, bom_query);

        let count: i64 = query.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 构建查询过滤条件
    #[allow(dead_code)]
    fn build_query_filter(
        query: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
        bom_query: &BomQuery,
    ) {
        if let Some(bom_name) = &bom_query.bom_name
            && !bom_name.is_empty()
        {
            if let Some(pattern) = build_fuzzy_pattern(bom_name) {
                query.push(" AND bom_name ILIKE ");
                query.push_bind(pattern);
            }
        }
        if let Some(create_by) = &bom_query.create_by
            && !create_by.is_empty()
        {
            query.push(" AND bom_detail ->> 'created_by' ILIKE ");
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
            query.push(" AND EXISTS (SELECT 1 FROM jsonb_array_elements(bom_detail->'nodes') AS node WHERE (node->>'product_id')::bigint = ");
            query.push_bind(product_id);
            query.push(")");
        }
        if let Some(product_code) = &bom_query.product_code
            && !product_code.is_empty()
        {
            query.push(" AND EXISTS (SELECT 1 FROM jsonb_array_elements(bom_detail->'nodes') AS node JOIN products p ON (node->>'product_id')::bigint = p.product_id WHERE (node->>'parent_id')::bigint = 0 AND p.meta->>'product_code' ILIKE ");
            query.push_bind(format!("%{}%", product_code));
            query.push(")");
        }
        if let Some(bom_category_id) = bom_query.bom_category_id {
            query.push(" AND bom_category_id = ");
            query.push_bind(bom_category_id);
        }
    }

    /// 查询使用指定产品的 BOM 列表
    /// 返回包含该产品的 BOM 的简要信息（最多 10 条）和总数
    pub async fn find_boms_using_product(
        pool: &PgPool,
        product_id: i64,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<ProductUsageResult> {
        let ps = page_size.unwrap_or(10) as i64;
        let offset = (page.unwrap_or(1).saturating_sub(1)) as i64 * ps;

        // 查询总数
        let total: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)
            FROM bom
            WHERE EXISTS (
                SELECT 1 FROM jsonb_array_elements(bom_detail->'nodes') AS node
                WHERE (node->>'product_id')::bigint = $1
            )
            "#,
            product_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        // 查询分页 BOM 信息
        let boms = sqlx::query_as!(
            BomReference,
            r#"
            SELECT bom_id, bom_name
            FROM bom
            WHERE EXISTS (
                SELECT 1 FROM jsonb_array_elements(bom_detail->'nodes') AS node
                WHERE (node->>'product_id')::bigint = $1
            )
            ORDER BY bom_name
            LIMIT $2 OFFSET $3
            "#,
            product_id,
            ps,
            offset
        )
        .fetch_all(pool)
        .await?;

        Ok(ProductUsageResult { boms, total })
    }

    /// 查询所有包含指定产品的完整 BOM 列表（用于物料替换）
    pub async fn find_all_boms_using_product(
        pool: &PgPool,
        product_id: i64,
    ) -> Result<Vec<Bom>> {
        let rows = sqlx::query_as::<_, Bom>(
            r#"
            SELECT bom_id, bom_name, create_at, update_at, bom_detail::text, bom_category_id
            FROM bom
            WHERE EXISTS (
                SELECT 1 FROM jsonb_array_elements(bom_detail->'nodes') AS node
                WHERE (node->>'product_id')::bigint = $1
            )
            "#,
        )
        .bind(product_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 批量查询哪些 product_code 有对应的 BOM（根节点）
    /// 返回有 BOM 的 product_code 集合
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
            JOIN bom b ON EXISTS (
                SELECT 1 FROM jsonb_array_elements(b.bom_detail->'nodes') AS node
                WHERE (node->>'product_id')::bigint = p.product_id
                AND (node->>'parent_id')::bigint = 0
            )
            WHERE p.meta->>'product_code' = ANY($1)
            "#,
        )
        .bind(product_codes)
        .fetch_all(pool)
        .await?;

        Ok(codes)
    }
}
