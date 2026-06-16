use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::{PageParams, PaginatedResult};

use super::model::*;

pub struct WorkCenterRepo;

impl WorkCenterRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        req: &CreateWorkCenterReq,
        operator_id: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO work_centers
                 (code, name, work_center_type, costs_hour, time_efficiency,
                  setup_time, cleanup_time, default_capacity, calendar_id, location, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               RETURNING id"#,
        )
        .bind(&req.code)
        .bind(&req.name)
        .bind(req.work_center_type)
        .bind(req.costs_hour)
        .bind(req.time_efficiency)
        .bind(req.setup_time)
        .bind(req.cleanup_time)
        .bind(req.default_capacity)
        .bind(req.calendar_id)
        .bind(&req.location)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn get_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<WorkCenter>> {
        let row = sqlx::query_as::<_, WorkCenter>(
            r#"SELECT id, code, name, work_center_type, costs_hour, time_efficiency,
                      setup_time, cleanup_time, default_capacity, calendar_id, location,
                      is_active, operator_id, created_at, updated_at
               FROM work_centers WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn get_by_code(
        &self,
        executor: PgExecutor<'_>,
        code: &str,
    ) -> Result<Option<WorkCenter>> {
        let row = sqlx::query_as::<_, WorkCenter>(
            r#"SELECT id, code, name, work_center_type, costs_hour, time_efficiency,
                      setup_time, cleanup_time, default_capacity, calendar_id, location,
                      is_active, operator_id, created_at, updated_at
               FROM work_centers WHERE code = $1 AND deleted_at IS NULL"#,
        )
        .bind(code)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn list(
        &self,
        executor: PgExecutor<'_>,
        filter: &WorkCenterFilter,
        page: &PageParams,
    ) -> Result<PaginatedResult<WorkCenter>> {
        let limit = page.page_size as i64;

        let mut where_clauses = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        if filter.keyword.is_some() {
            where_clauses.push(format!(
                "(code ILIKE ${param_idx} OR name ILIKE ${param_idx})"
            ));
            param_idx += 1;
        }
        if filter.work_center_type.is_some() {
            where_clauses.push(format!("work_center_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.is_active.is_some() {
            where_clauses.push(format!("is_active = ${param_idx}"));
            param_idx += 1;
        }

        let where_sql = where_clauses.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM work_centers WHERE {where_sql}");
        let list_sql = format!(
            "SELECT id, code, name, work_center_type, costs_hour, time_efficiency, \
             setup_time, cleanup_time, default_capacity, calendar_id, location, \
             is_active, operator_id, created_at, updated_at \
             FROM work_centers WHERE {where_sql} ORDER BY id DESC LIMIT ${param_idx} OFFSET ${}",
            param_idx + 1
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut list_q = sqlx::query_as::<_, WorkCenter>(sqlx::AssertSqlSafe(list_sql));

        if let Some(kw) = &filter.keyword {
            let pattern = format!("%{kw}%");
            count_q = count_q.bind(pattern.clone());
            list_q = list_q.bind(pattern);
        }
        if let Some(wc_type) = filter.work_center_type {
            count_q = count_q.bind(wc_type);
            list_q = list_q.bind(wc_type);
        }
        if let Some(active) = filter.is_active {
            count_q = count_q.bind(active);
            list_q = list_q.bind(active);
        }

        let total = count_q.fetch_one(&mut *executor).await? as u64;
        list_q = list_q.bind(limit).bind(page.offset() as i64);
        let items = list_q.fetch_all(executor).await?;

        let total_pages = if page.page_size > 0 {
            ((total as f64) / (page.page_size as f64)).ceil() as u32
        } else {
            0
        };

        Ok(PaginatedResult {
            items,
            total,
            page: page.page,
            page_size: page.page_size,
            total_pages,
        })
    }

    pub async fn list_active(&self, executor: PgExecutor<'_>) -> Result<Vec<WorkCenter>> {
        let rows = sqlx::query_as::<_, WorkCenter>(
            r#"SELECT id, code, name, work_center_type, costs_hour, time_efficiency,
                      setup_time, cleanup_time, default_capacity, calendar_id, location,
                      is_active, operator_id, created_at, updated_at
               FROM work_centers WHERE is_active = true AND deleted_at IS NULL ORDER BY id"#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateWorkCenterReq,
    ) -> Result<()> {
        let mut sets = vec!["updated_at = NOW()".to_string()];
        let mut param_idx = 2u32;

        if req.name.is_some() {
            sets.push(format!("name = ${param_idx}"));
            param_idx += 1;
        }
        if req.work_center_type.is_some() {
            sets.push(format!("work_center_type = ${param_idx}"));
            param_idx += 1;
        }
        if req.costs_hour.is_some() {
            sets.push(format!("costs_hour = ${param_idx}"));
            param_idx += 1;
        }
        if req.time_efficiency.is_some() {
            sets.push(format!("time_efficiency = ${param_idx}"));
            param_idx += 1;
        }
        if req.setup_time.is_some() {
            sets.push(format!("setup_time = ${param_idx}"));
            param_idx += 1;
        }
        if req.cleanup_time.is_some() {
            sets.push(format!("cleanup_time = ${param_idx}"));
            param_idx += 1;
        }
        if req.default_capacity.is_some() {
            sets.push(format!("default_capacity = ${param_idx}"));
            param_idx += 1;
        }
        if req.calendar_id.is_some() {
            sets.push(format!("calendar_id = ${param_idx}"));
            param_idx += 1;
        }
        if req.location.is_some() {
            sets.push(format!("location = ${param_idx}"));
            param_idx += 1;
        }
        if req.is_active.is_some() {
            sets.push(format!("is_active = ${param_idx}"));
            param_idx += 1;
        }

        let sql = format!(
            "UPDATE work_centers SET {} WHERE id = $1 AND deleted_at IS NULL",
            sets.join(", ")
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(v) = &req.name {
            q = q.bind(v);
        }
        if let Some(v) = req.work_center_type {
            q = q.bind(v);
        }
        if let Some(v) = req.costs_hour {
            q = q.bind(v);
        }
        if let Some(v) = req.time_efficiency {
            q = q.bind(v);
        }
        if let Some(v) = req.setup_time {
            q = q.bind(v);
        }
        if let Some(v) = req.cleanup_time {
            q = q.bind(v);
        }
        if let Some(v) = req.default_capacity {
            q = q.bind(v);
        }
        if let Some(v) = req.calendar_id {
            q = q.bind(v);
        }
        if let Some(v) = &req.location {
            q = q.bind(v);
        }
        if let Some(v) = req.is_active {
            q = q.bind(v);
        }

        q.execute(&mut *executor).await?;
        Ok(())
    }

    pub async fn soft_delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE work_centers SET deleted_at = NOW(), is_active = false WHERE id = $1")
            .bind(id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }
}
