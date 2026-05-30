use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub struct ProductRepo;

impl ProductRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        product_code: &str,
        req: &CreateProductReq,
    ) -> Result<i64> {
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
    ) -> Result<()> {
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
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

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

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE products SET deleted_at = NOW() WHERE product_id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<Product>> {
        let product = sqlx::query_as::<sqlx::Postgres, Product>(
            "SELECT product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta, created_at, updated_at, deleted_at FROM products WHERE product_id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(product)
    }

    pub async fn find_by_ids(&self, executor: PgExecutor<'_>, ids: Vec<i64>) -> Result<Vec<Product>> {
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
    ) -> Result<PaginatedResult<Product>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let name_params: Vec<String> = if let Some(ref name) = filter.name {
            let tokens: Vec<&str> = name.split_whitespace().collect();
            let mut params = Vec::new();
            for token in tokens {
                param_idx += 1;
                conditions.push(format!("pdt_name ILIKE ${param_idx}"));
                params.push(format!("%{token}%"));
            }
            params
        } else { Vec::new() };

        let code_param = if let Some(ref code) = filter.code {
            param_idx += 1;
            conditions.push(format!("product_code ILIKE ${param_idx}"));
            Some(code.clone())
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

        let category_param = if let Some(cat_id) = filter.category_id {
            param_idx += 1;
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM product_categories pc WHERE pc.product_id = products.product_id AND pc.category_id = ${param_idx})"
            ));
            Some(cat_id)
        } else { None };
        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM products WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        for v in &name_params { count_q = count_q.bind(v); }
        if let Some(ref v) = code_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(v) = dept_param { count_q = count_q.bind(v); }
        if let Some(v) = category_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta, created_at, updated_at, deleted_at FROM products WHERE {where_clause} ORDER BY product_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Product>(sqlx::AssertSqlSafe(data_sql));
        for v in &name_params { data_q = data_q.bind(v); }
        if let Some(ref v) = code_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(v) = dept_param { data_q = data_q.bind(v); }
        if let Some(v) = category_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn check_code_unique(&self, executor: PgExecutor<'_>, code: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM products WHERE product_code = $1 AND deleted_at IS NULL",
        )
        .bind(code)
        .fetch_one(executor)
        .await?;
        Ok(count == 0)
    }

    /// Batch query products by codes — Excel import support
    pub async fn find_by_codes(executor: PgExecutor<'_>, codes: &[String]) -> Result<Vec<Product>> {
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

    pub async fn count_product_usage_in_boms(db: PgExecutor<'_>, product_id: i64) -> Result<i64> {
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(DISTINCT b.bom_id) FROM bom_nodes bn JOIN boms b ON bn.bom_id = b.bom_id WHERE bn.product_id = $1 AND b.deleted_at IS NULL"#,
        )
        .bind(product_id)
        .fetch_one(&mut *db)
        .await?;
        Ok(total)
    }

    pub async fn query_product_usage_in_boms(db: PgExecutor<'_>, product_id: i64, limit: i64, offset: i64) -> Result<Vec<UsageEntry>> {
        let items = sqlx::query_as::<sqlx::Postgres, UsageEntry>(
            r#"SELECT
                'bom' AS source_type,
                b.bom_id AS source_id,
                b.bom_name AS source_name,
                b.status AS bom_status,
                b.version AS bom_version,
                bn.quantity,
                bn.unit AS node_unit,
                bn.remark AS node_remark,
                root_p.pdt_name AS parent_product_name,
                root_p.product_code AS parent_product_code,
                b.update_at AS bom_updated_at
            FROM bom_nodes bn
            JOIN boms b ON bn.bom_id = b.bom_id
            LEFT JOIN LATERAL (
                SELECT bn2.product_id
                FROM bom_nodes bn2
                WHERE bn2.bom_id = b.bom_id AND bn2.parent_id = 0
                ORDER BY bn2.order_num
                LIMIT 1
            ) root ON TRUE
            LEFT JOIN products root_p ON root.product_id = root_p.product_id
            WHERE bn.product_id = $1 AND b.deleted_at IS NULL
            GROUP BY b.bom_id, b.bom_name, b.status, b.version,
                     bn.quantity, bn.unit, bn.remark,
                     root_p.pdt_name, root_p.product_code,
                     b.update_at
            ORDER BY b.bom_id DESC
            LIMIT $2 OFFSET $3"#,
        )
        .bind(product_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
        Ok(items)
    }

    /// Update product name by id — Excel import support
    pub async fn update_name(executor: PgExecutor<'_>, id: i64, name: &str) -> Result<()> {
        sqlx::query("UPDATE products SET pdt_name = $2 WHERE product_id = $1")
            .bind(id)
            .bind(name)
            .execute(executor)
            .await?;
        Ok(())
    }
}
