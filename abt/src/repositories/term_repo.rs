//! 分类数据访问层
//!
//! 提供分类的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{Term, TermTree};
use crate::repositories::Executor;

/// 分类数据仓库
pub struct TermRepo;

#[allow(dead_code)]
impl TermRepo {
    /// 创建新分类
    pub async fn insert(
        executor: Executor<'_>,
        term_name: &str,
        term_parent: i64,
        taxonomy: &str,
    ) -> Result<i64> {
        let term_id: i64 = sqlx::query_scalar!(
            r#"
            INSERT INTO terms (term_name, term_parent, taxonomy, term_meta)
            VALUES ($1, $2, $3, '{"count": 0}'::jsonb)
            RETURNING term_id
            "#,
            term_name,
            term_parent,
            taxonomy
        )
        .fetch_one(executor)
        .await?;

        Ok(term_id)
    }

    /// 更新分类名称
    pub async fn update_name(executor: Executor<'_>, term_id: i64, term_name: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE terms
            SET term_name = $1
            WHERE term_id = $2
            "#,
            term_name,
            term_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 删除分类
    pub async fn delete(executor: Executor<'_>, term_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM terms WHERE term_id = $1", term_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    /// 根据 ID 查找分类
    pub async fn find_by_id(pool: &PgPool, term_id: i64) -> Result<Option<Term>> {
        let row = sqlx::query_as::<_, Term>(
            "SELECT term_id, term_name, term_parent, taxonomy, term_meta FROM terms WHERE term_id = $1",
        )
        .bind(term_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 获取指定分类法的所有分类
    pub async fn list_by_taxonomy(pool: &PgPool, taxonomy: &str) -> Result<Vec<Term>> {
        let rows = sqlx::query_as::<_, Term>(
            "SELECT term_id, term_name, term_parent, taxonomy, term_meta FROM terms WHERE taxonomy = $1 ORDER BY term_id",
        )
        .bind(taxonomy)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 获取子分类
    pub async fn get_children(pool: &PgPool, parent_id: i64) -> Result<Vec<Term>> {
        let rows = sqlx::query_as::<_, Term>(
            "SELECT term_id, term_name, term_parent, taxonomy, term_meta FROM terms WHERE term_parent = $1 ORDER BY term_id",
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 构建分类树
    pub fn build_tree(terms: &[Term], parent_id: i64) -> Vec<TermTree> {
        terms
            .iter()
            .filter(|t| t.term_parent == parent_id)
            .map(|term| {
                let mut tree: TermTree = term.clone().into();
                tree.children = Self::build_tree(terms, term.term_id);
                tree
            })
            .collect()
    }

    /// 获取分类树
    pub async fn get_tree(pool: &PgPool, taxonomy: &str) -> Result<Vec<TermTree>> {
        let terms = Self::list_by_taxonomy(pool, taxonomy).await?;
        Ok(Self::build_tree(&terms, 0))
    }

    /// 更新分类计数
    pub async fn update_count(executor: Executor<'_>, term_id: i64, count: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE terms
            SET term_meta = jsonb_set(term_meta, '{count}', $1::jsonb)
            WHERE term_id = $2
            "#,
        )
        .bind(count)
        .bind(term_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 检查分类是否有子分类
    pub async fn has_children(pool: &PgPool, term_id: i64) -> Result<bool> {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM terms WHERE term_parent = $1",
        )
        .bind(term_id)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// 根据名称和分类法查找分类
    pub async fn find_by_name(
        pool: &PgPool,
        name: &str,
        taxonomy: &str,
        parent_id: i64,
    ) -> Result<Option<Term>> {
        let row = sqlx::query_as::<_, Term>(
            r#"
            SELECT term_id, term_name, term_parent, taxonomy, term_meta
            FROM terms
            WHERE term_name = $1 AND taxonomy = $2 AND term_parent = $3
            "#,
        )
        .bind(name)
        .bind(taxonomy)
        .bind(parent_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }
}
