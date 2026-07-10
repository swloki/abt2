use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub struct LaborProcessDictRepo;

impl LaborProcessDictRepo {
    pub async fn create(&self, executor: PgExecutor<'_>, code: &str, req: &CreateLaborProcessDictReq, operator_id: i64) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO labor_process_dicts (code, name, description, sort_order, default_work_center_id, default_standard_time, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id"#,
        )
        .bind(code)
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.sort_order)
        .bind(req.default_work_center_id)
        .bind(req.default_standard_time)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    #[allow(unused_assignments)]
    pub async fn update(&self, executor: PgExecutor<'_>, id: i64, req: &UpdateLaborProcessDictReq, operator_id: i64) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.name.is_some() { sets.push(format!("name = ${param_idx}")); param_idx += 1; }
        if req.description.is_some() { sets.push(format!("description = ${param_idx}")); param_idx += 1; }
        if req.sort_order.is_some() { sets.push(format!("sort_order = ${param_idx}")); param_idx += 1; }
        if req.default_work_center_id.is_some() { sets.push(format!("default_work_center_id = ${param_idx}")); param_idx += 1; }
        if req.default_standard_time.is_some() { sets.push(format!("default_standard_time = ${param_idx}")); param_idx += 1; }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        sets.push(format!("operator_id = ${param_idx}"));
        let sql = format!("UPDATE labor_process_dicts SET {} WHERE id = $1 AND deleted_at IS NULL", sets.join(", "));
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(ref v) = req.name { q = q.bind(v); }
        if let Some(ref v) = req.description { q = q.bind(v); }
        if let Some(v) = req.sort_order { q = q.bind(v); }
        if let Some(v) = req.default_work_center_id { q = q.bind(v); }
        if let Some(v) = req.default_standard_time { q = q.bind(v); }
        q = q.bind(operator_id);

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE labor_process_dicts SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<LaborProcessDict>> {
        let row = sqlx::query_as::<sqlx::Postgres, LaborProcessDict>(
            "SELECT id, code, name, description, sort_order, default_work_center_id, default_standard_time, operator_id, created_at, updated_at, deleted_at FROM labor_process_dicts WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn exists_routing_step_reference(&self, executor: PgExecutor<'_>, code: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM routing_steps WHERE process_code = $1",
        )
        .bind(code)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }

    #[allow(unused_assignments)]
    pub async fn query(&self, executor: PgExecutor<'_>, filter: &LaborProcessDictQuery, page: &PageParams) -> Result<PaginatedResult<LaborProcessDict>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        let keyword_param = if let Some(ref kw) = filter.keyword {
            conditions.push(format!("(name ILIKE ${param_idx} OR code ILIKE ${param_idx})"));
            param_idx += 1;
            Some(format!("%{kw}%"))
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM labor_process_dicts WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;

        let data_sql = format!(
            "SELECT id, code, name, description, sort_order, default_work_center_id, default_standard_time, operator_id, created_at, updated_at, deleted_at FROM labor_process_dicts WHERE {where_clause} ORDER BY sort_order, id LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, LaborProcessDict>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref v) = keyword_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// Return codes that already exist in the DB (for Excel import deduplication)
    pub async fn find_existing_codes(&self, executor: PgExecutor<'_>, codes: &[String]) -> Result<Vec<String>> {
        if codes.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query_scalar::<sqlx::Postgres, String>(
            "SELECT code FROM labor_process_dicts WHERE code = ANY($1) AND deleted_at IS NULL",
        )
        .bind(codes)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// List all active labor process dicts (for Excel export)
    pub async fn list_all(&self, executor: PgExecutor<'_>) -> Result<Vec<LaborProcessDict>> {
        let rows = sqlx::query_as::<sqlx::Postgres, LaborProcessDict>(
            "SELECT id, code, name, description, sort_order, operator_id, created_at, updated_at, deleted_at \
             FROM labor_process_dicts WHERE deleted_at IS NULL ORDER BY sort_order",
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
