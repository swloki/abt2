use crate::shared::types::PgExecutor;
use crate::shared::types::RepoResult;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub struct ProductRepo;

impl ProductRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        req: &CreateProductReq,
    ) -> RepoResult<i64> {
        let meta_json = serde_json::to_value(&req.meta)?;
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO products (pdt_name, product_code, unit, status, external_code, owner_department_id, meta)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING product_id"#,
        )
        .bind(&req.name)
        .bind(product_code)
        .bind(&req.unit)
        .bind(req.status.as_i16())
        .bind(&req.external_code)
        .bind(req.owner_department_id)
        .bind(&meta_json)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateProductReq,
    ) -> RepoResult<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.name.is_some() { sets.push(format!("pdt_name = ${param_idx}")); param_idx += 1; }
        if req.unit.is_some() { sets.push(format!("unit = ${param_idx}")); param_idx += 1; }
        if req.external_code.is_some() { sets.push(format!("external_code = ${param_idx}")); param_idx += 1; }
        if req.owner_department_id.is_some() { sets.push(format!("owner_department_id = ${param_idx}")); param_idx += 1; }
        if req.meta.is_some() { sets.push(format!("meta = ${param_idx}")); param_idx += 1; }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let sql = format!("UPDATE products SET {} WHERE product_id = $1 AND deleted_at IS NULL", sets.join(", "));
        let mut q = sqlx::query(&sql).bind(id);

        if let Some(ref v) = req.name { q = q.bind(v); }
        if let Some(ref v) = req.unit { q = q.bind(v); }
        if let Some(ref v) = req.external_code { q = q.bind(v); }
        if let Some(v) = req.owner_department_id { q = q.bind(v); }
        if let Some(ref v) = req.meta {
            let json = serde_json::to_value(v)?;
            q = q.bind(json);
        }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> RepoResult<()> {
        sqlx::query("UPDATE products SET deleted_at = NOW() WHERE product_id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> RepoResult<Option<Product>> {
        let product = sqlx::query_as::<sqlx::Postgres, Product>(
            "SELECT product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta, created_at, updated_at, deleted_at FROM products WHERE product_id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(product)
    }

    pub async fn find_by_ids(&self, executor: PgExecutor<'_>, ids: Vec<i64>) -> RepoResult<Vec<Product>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let products = sqlx::query_as::<sqlx::Postgres, Product>(
            "SELECT product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta, created_at, updated_at, deleted_at FROM products WHERE product_id = ANY($1) AND deleted_at IS NULL",
        )
        .bind(&ids)
        .fetch_all(executor)
        .await?;
        Ok(products)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &ProductQuery,
        page: &PageParams,
    ) -> RepoResult<PaginatedResult<Product>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("pdt_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else { None };

        let code_param = if let Some(ref code) = filter.code {
            param_idx += 1;
            conditions.push(format!("product_code ILIKE ${param_idx}"));
            Some(format!("%{code}%"))
        } else { None };

        let status_param = if let Some(status) = filter.status {
            param_idx += 1;
            conditions.push(format!("status = ${param_idx}"));
            Some(status.as_i16())
        } else { None };

        let dept_param = if let Some(dept_id) = filter.owner_department_id {
            param_idx += 1;
            conditions.push(format!("owner_department_id = ${param_idx}"));
            Some(dept_id)
        } else { None };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM products WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        if let Some(ref v) = code_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(v) = dept_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta, created_at, updated_at, deleted_at FROM products WHERE {where_clause} ORDER BY product_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Product>(&data_sql);
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        if let Some(ref v) = code_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(v) = dept_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn check_code_unique(&self, executor: PgExecutor<'_>, code: &str) -> RepoResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM products WHERE product_code = $1 AND deleted_at IS NULL",
        )
        .bind(code)
        .fetch_one(executor)
        .await?;
        Ok(count == 0)
    }

    /// Batch query products by codes — Excel import support
    pub async fn find_by_codes(executor: PgExecutor<'_>, codes: &[String]) -> RepoResult<Vec<Product>> {
        if codes.is_empty() {
            return Ok(vec![]);
        }
        let products = sqlx::query_as::<sqlx::Postgres, Product>(
            "SELECT product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta, created_at, updated_at, deleted_at FROM products WHERE product_code = ANY($1) AND deleted_at IS NULL",
        )
        .bind(codes)
        .fetch_all(executor)
        .await?;
        Ok(products)
    }

    /// Update product name by id — Excel import support
    pub async fn update_name(executor: PgExecutor<'_>, id: i64, name: &str) -> RepoResult<()> {
        sqlx::query("UPDATE products SET pdt_name = $2 WHERE product_id = $1")
            .bind(id)
            .bind(name)
            .execute(executor)
            .await?;
        Ok(())
    }
}
