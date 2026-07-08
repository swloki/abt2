use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

pub struct RoutingRepo;

impl RoutingRepo {
    pub async fn create(&self, executor: PgExecutor<'_>, code: &str, req: &CreateRoutingReq, operator_id: i64) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO routings (code, name, description, operator_id)
               VALUES ($1, $2, $3, $4)
               RETURNING id"#,
        )
        .bind(code)
        .bind(&req.name)
        .bind(&req.description)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn insert_steps(&self, executor: PgExecutor<'_>, routing_id: i64, steps: &[RoutingStepInput]) -> Result<()> {
        for step in steps {
            sqlx::query(
                r#"INSERT INTO routing_steps (routing_id, process_code, step_order, is_required, remark,
                   work_center_id, standard_time, standard_cost,
                   allowed_loss_rate, is_outsourced, is_inspection_point)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
            )
            .bind(routing_id)
            .bind(&step.process_code)
            .bind(step.step_order)
            .bind(step.is_required)
            .bind(&step.remark)
            .bind(step.work_center_id)
            .bind(step.standard_time)
            .bind(step.standard_cost)
            .bind(step.allowed_loss_rate)
            .bind(step.is_outsourced)
            .bind(step.is_inspection_point)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn delete_steps(&self, executor: PgExecutor<'_>, routing_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM routing_steps WHERE routing_id = $1")
            .bind(routing_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    #[allow(unused_assignments)]
    pub async fn update(&self, executor: PgExecutor<'_>, id: i64, req: &UpdateRoutingReq, operator_id: i64) -> Result<()> {
        let mut sets = vec!["updated_at = NOW()".to_string()];
        let mut param_idx = 2u32;

        if req.name.is_some() { sets.push(format!("name = ${param_idx}")); param_idx += 1; }
        if req.description.is_some() { sets.push(format!("description = ${param_idx}")); param_idx += 1; }

        sets.push(format!("operator_id = ${param_idx}"));
        let sql = format!("UPDATE routings SET {} WHERE id = $1 AND deleted_at IS NULL", sets.join(", "));
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(ref v) = req.name { q = q.bind(v); }
        if let Some(ref v) = req.description { q = q.bind(v); }
        q = q.bind(operator_id);

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE routings SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<Routing>> {
        let row = sqlx::query_as::<sqlx::Postgres, Routing>(
            "SELECT id, code, name, description, operator_id, created_at, updated_at, deleted_at FROM routings WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn find_steps(&self, executor: PgExecutor<'_>, routing_id: i64) -> Result<Vec<RoutingStep>> {
        let steps = sqlx::query_as::<sqlx::Postgres, RoutingStep>(
            "SELECT rs.id, rs.routing_id, rs.process_code, rs.step_order, rs.is_required, rs.remark, rs.created_at, lpd.name AS process_name, rs.work_center_id, rs.standard_time, rs.standard_cost, rs.allowed_loss_rate, rs.is_outsourced, rs.is_inspection_point FROM routing_steps rs LEFT JOIN labor_process_dicts lpd ON rs.process_code = lpd.code WHERE rs.routing_id = $1 ORDER BY rs.step_order",
        )
        .bind(routing_id)
        .fetch_all(executor)
        .await?;
        Ok(steps)
    }

    pub async fn query(&self, executor: PgExecutor<'_>, filter: &RoutingQuery, page: &PageParams) -> Result<PaginatedResult<Routing>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let keyword_param = if let Some(ref kw) = filter.keyword {
            param_idx += 1;
            conditions.push(format!("name ILIKE ${param_idx}"));
            Some(format!("%{kw}%"))
        } else {
            None
        };

        let bom_keyword_param = if let Some(ref kw) =
            filter.bom_keyword.as_ref().filter(|s| !s.trim().is_empty())
        {
            param_idx += 1;
            conditions.push(format!(
                "id IN (SELECT br.routing_id FROM bom_routings br LEFT JOIN products p ON br.product_code = p.product_code WHERE br.product_code ILIKE ${param_idx} OR p.pdt_name ILIKE ${param_idx})"
            ));
            Some(format!("%{}%", kw.trim()))
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM routings WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        if let Some(ref v) = bom_keyword_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;

        let data_sql = format!(
            "SELECT id, code, name, description, operator_id, created_at, updated_at, deleted_at FROM routings WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Routing>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref v) = keyword_param { data_q = data_q.bind(v); }
        if let Some(ref v) = bom_keyword_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn find_matching_by_process_codes(&self, executor: PgExecutor<'_>, process_codes: &[String]) -> Result<Option<i64>> {
        if process_codes.is_empty() {
            return Ok(None);
        }
        let unique_codes: Vec<String> = {
            let mut codes = process_codes.to_vec();
            codes.sort();
            codes.dedup();
            codes
        };
        let unique_len = unique_codes.len() as i64;
        let routing_id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"SELECT r.id
               FROM routings r
               WHERE r.deleted_at IS NULL
                 AND (SELECT COUNT(DISTINCT rs.process_code)
                      FROM routing_steps rs
                      WHERE rs.routing_id = r.id
                        AND rs.process_code = ANY($1)
                     ) = $2
                 AND (SELECT COUNT(*) FROM routing_steps rs WHERE rs.routing_id = r.id) = $2
               ORDER BY r.id
               LIMIT 1"#,
        )
        .bind(&unique_codes)
        .bind(unique_len)
        .fetch_optional(executor)
        .await?;
        Ok(routing_id)
    }

    pub async fn set_bom_routing(&self, executor: PgExecutor<'_>, product_code: &str, routing_id: i64, operator_id: i64) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO bom_routings (product_code, routing_id, operator_id)
               VALUES ($1, $2, $3)
               ON CONFLICT (product_code) DO UPDATE SET routing_id = $2, operator_id = $3, updated_at = NOW()"#,
        )
        .bind(product_code)
        .bind(routing_id)
        .bind(operator_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn get_bom_routing(&self, executor: PgExecutor<'_>, product_code: &str) -> Result<Option<BomRouting>> {
        let row = sqlx::query_as::<sqlx::Postgres, BomRouting>(
            "SELECT b.id, b.product_code, b.routing_id, b.operator_id, b.created_at, b.updated_at, p.pdt_name AS product_name FROM bom_routings b LEFT JOIN products p ON b.product_code = p.product_code WHERE b.product_code = $1",
        )
        .bind(product_code)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn delete_bom_routing(&self, executor: PgExecutor<'_>, product_code: &str) -> Result<()> {
        sqlx::query("DELETE FROM bom_routings WHERE product_code = $1")
            .bind(product_code)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn paginate_boms_by_routing(&self, executor: PgExecutor<'_>, routing_id: i64, keyword: Option<&str>, page: &PageParams) -> Result<PaginatedResult<BomRouting>> {
        let mut conditions = vec!["b.routing_id = $1".to_string()];
        let mut param_idx = 1u32;
        let keyword_param = if let Some(kw) = keyword.filter(|s| !s.trim().is_empty()) {
            param_idx += 1;
            conditions.push(format!("(b.product_code ILIKE ${param_idx} OR p.pdt_name ILIKE ${param_idx})"));
            Some(format!("%{}%", kw.trim()))
        } else {
            None
        };
        let where_clause = conditions.join(" AND ");

        let count_sql = format!(
            "SELECT COUNT(*) FROM bom_routings b LEFT JOIN products p ON b.product_code = p.product_code WHERE {where_clause}"
        );
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        count_q = count_q.bind(routing_id);
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;

        let data_sql = format!(
            "SELECT b.id, b.product_code, b.routing_id, b.operator_id, b.created_at, b.updated_at, p.pdt_name AS product_name FROM bom_routings b LEFT JOIN products p ON b.product_code = p.product_code WHERE {where_clause} ORDER BY b.id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, BomRouting>(sqlx::AssertSqlSafe(data_sql));
        data_q = data_q.bind(routing_id);
        if let Some(ref v) = keyword_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    pub async fn list_boms_by_routing(&self, executor: PgExecutor<'_>, routing_id: i64) -> Result<Vec<BomRouting>> {
        let rows = sqlx::query_as::<sqlx::Postgres, BomRouting>(
            "SELECT b.id, b.product_code, b.routing_id, b.operator_id, b.created_at, b.updated_at, p.pdt_name AS product_name FROM bom_routings b LEFT JOIN products p ON b.product_code = p.product_code WHERE b.routing_id = $1",
        )
        .bind(routing_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
