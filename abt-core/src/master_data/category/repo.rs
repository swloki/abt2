use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub struct CategoryRepo;

impl CategoryRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        category_name: &str,
        parent_id: i64,
        path: &str,
        meta: &CategoryMeta,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "INSERT INTO categories (category_name, parent_id, path, meta) VALUES ($1, $2, $3, $4) RETURNING category_id",
        )
        .bind(category_name)
        .bind(parent_id)
        .bind(path)
        .bind(meta)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn update(&self, executor: PgExecutor<'_>, id: i64, req: &UpdateCategoryReq) -> Result<()> {
        if let Some(ref name) = req.category_name {
            sqlx::query("UPDATE categories SET category_name = $1, updated_at = NOW() WHERE category_id = $2")
                .bind(name)
                .bind(id)
                .execute(executor)
                .await?;
        }
        Ok(())
    }

    pub async fn update_path(&self, executor: PgExecutor<'_>, id: i64, new_path: &str) -> Result<()> {
        sqlx::query("UPDATE categories SET path = $1 WHERE category_id = $2")
            .bind(new_path)
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn update_parent(&self, executor: PgExecutor<'_>, id: i64, new_parent_id: i64) -> Result<()> {
        sqlx::query("UPDATE categories SET parent_id = $1, updated_at = NOW() WHERE category_id = $2")
            .bind(new_parent_id)
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn update_path_subtree(&self, executor: PgExecutor<'_>, old_prefix: &str, new_prefix: &str) -> Result<()> {
        let old_len = old_prefix.len() as i32;
        sqlx::query(
            "UPDATE categories SET path = $1 || substring(path, $2 + 1) WHERE path LIKE $3 || '%'",
        )
        .bind(new_prefix)
        .bind(old_len)
        .bind(old_prefix)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM categories WHERE category_id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<Category>> {
        let cat = sqlx::query_as::<sqlx::Postgres, Category>(
            "SELECT category_id, category_name, parent_id, path, meta, created_at, updated_at FROM categories WHERE category_id = $1",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(cat)
    }

    pub async fn find_all(&self, executor: PgExecutor<'_>) -> Result<Vec<Category>> {
        let cats = sqlx::query_as::<sqlx::Postgres, Category>(
            "SELECT category_id, category_name, parent_id, path, meta, created_at, updated_at FROM categories ORDER BY path",
        )
        .fetch_all(executor)
        .await?;
        Ok(cats)
    }

    #[allow(unused_assignments)]
    pub async fn query(&self, executor: PgExecutor<'_>, filter: &CategoryQuery, page: &PageParams) -> Result<PaginatedResult<Category>> {
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("category_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else { None };

        let parent_param = if let Some(pid) = filter.parent_id {
            param_idx += 1;
            conditions.push(format!("parent_id = ${param_idx}"));
            Some(pid)
        } else { None };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM categories WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        if let Some(v) = parent_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT category_id, category_name, parent_id, path, meta, created_at, updated_at FROM categories WHERE {where_clause} ORDER BY path LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Category>(&data_sql);
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        if let Some(v) = parent_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn find_children_count(&self, executor: PgExecutor<'_>, id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM categories WHERE parent_id = $1",
        )
        .bind(id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }

    pub async fn find_products_count(&self, executor: PgExecutor<'_>, id: i64) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM product_categories WHERE category_id = $1",
        )
        .bind(id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }

    pub async fn assign_products(&self, executor: PgExecutor<'_>, category_id: i64, product_ids: &[i64]) -> Result<()> {
        for pid in product_ids {
            sqlx::query(
                "INSERT INTO product_categories (product_id, category_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(pid)
            .bind(category_id)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn remove_products(&self, executor: PgExecutor<'_>, category_id: i64, product_ids: &[i64]) -> Result<()> {
        sqlx::query("DELETE FROM product_categories WHERE category_id = $1 AND product_id = ANY($2)")
            .bind(category_id)
            .bind(product_ids)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn update_meta_count(&self, executor: PgExecutor<'_>, category_id: i64, count: i64) -> Result<()> {
        let meta = CategoryMeta { count };
        sqlx::query("UPDATE categories SET meta = $1 WHERE category_id = $2")
            .bind(&meta)
            .bind(category_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// Replace all category assignments for a given product (Excel import support).
    /// Deletes existing rows then inserts the new set.
    pub async fn sync_product_categories(
        &self,
        executor: PgExecutor<'_>,
        product_id: i64,
        category_ids: &[i64],
    ) -> Result<()> {
        sqlx::query("DELETE FROM product_categories WHERE product_id = $1")
            .bind(product_id)
            .execute(&mut *executor)
            .await?;

        for cid in category_ids {
            sqlx::query(
                "INSERT INTO product_categories (product_id, category_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(product_id)
            .bind(cid)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }
}
