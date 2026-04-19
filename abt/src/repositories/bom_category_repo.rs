use anyhow::Result;
use sqlx::PgPool;

use crate::models::{BomCategory, BomCategoryQuery, CreateBomCategoryRequest, UpdateBomCategoryRequest};
use crate::repositories::Executor;

pub struct BomCategoryRepo;

impl BomCategoryRepo {
    pub async fn insert(
        executor: Executor<'_>,
        req: &CreateBomCategoryRequest,
    ) -> Result<i64> {
        let bom_category_id = sqlx::query_scalar!(
            r#"
            INSERT INTO bom_category (bom_category_name)
            VALUES ($1)
            RETURNING bom_category_id
            "#,
            req.bom_category_name
        )
        .fetch_one(executor)
        .await?;

        Ok(bom_category_id)
    }

    pub async fn update(
        executor: Executor<'_>,
        bom_category_id: i64,
        req: &UpdateBomCategoryRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE bom_category
            SET bom_category_name = $2
            WHERE bom_category_id = $1
            "#,
            bom_category_id,
            req.bom_category_name
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_>, bom_category_id: i64) -> Result<()> {
        sqlx::query!(
            "DELETE FROM bom_category WHERE bom_category_id = $1",
            bom_category_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, bom_category_id: i64) -> Result<Option<BomCategory>> {
        let category = sqlx::query_as!(
            BomCategory,
            r#"
            SELECT bom_category_id, bom_category_name, created_at
            FROM bom_category
            WHERE bom_category_id = $1
            "#,
            bom_category_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(category)
    }

    pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<BomCategory>> {
        let category = sqlx::query_as!(
            BomCategory,
            r#"
            SELECT bom_category_id, bom_category_name, created_at
            FROM bom_category
            WHERE bom_category_name = $1
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(category)
    }

    pub async fn query(pool: &PgPool, query: &BomCategoryQuery) -> Result<Vec<BomCategory>> {
        let mut sql_query = sqlx::QueryBuilder::new(
            r#"
            SELECT bom_category_id, bom_category_name, created_at
            FROM bom_category
            WHERE 1=1
            "#
        );

        if let Some(keyword) = &query.keyword {
            if !keyword.is_empty() {
                sql_query.push(" AND bom_category_name ILIKE ");
                sql_query.push_bind(format!("%{}%", keyword));
            }
        }

        sql_query.push(" ORDER BY bom_category_id DESC");

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        sql_query.push(" LIMIT ");
        sql_query.push_bind(page_size as i32);
        sql_query.push(" OFFSET ");
        sql_query.push_bind(((page - 1) * page_size) as i32);

        let categories = sql_query.build_query_as::<BomCategory>().fetch_all(pool).await?;

        Ok(categories)
    }

    pub async fn query_count(pool: &PgPool, query: &BomCategoryQuery) -> Result<i64> {
        let mut sql_query = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM bom_category WHERE 1=1"
        );

        if let Some(keyword) = &query.keyword {
            if !keyword.is_empty() {
                sql_query.push(" AND bom_category_name ILIKE ");
                sql_query.push_bind(format!("%{}%", keyword));
            }
        }

        let count: i64 = sql_query.build_query_scalar().fetch_one(pool).await?;

        Ok(count)
    }

    pub async fn is_name_exists(pool: &PgPool, name: &str) -> Result<bool> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bom_category WHERE bom_category_name = $1)",
        )
        .bind(name)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    pub async fn has_boms(pool: &PgPool, bom_category_id: i64) -> Result<bool> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bom WHERE bom_category_id = $1)",
        )
        .bind(bom_category_id)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }
}
