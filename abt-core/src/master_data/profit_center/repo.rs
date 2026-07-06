use crate::shared::types::{PgExecutor, Result};

use super::model::*;

pub struct ProfitCenterRepo;

impl ProfitCenterRepo {
    pub async fn list_active(executor: PgExecutor<'_>) -> Result<Vec<ProfitCenter>> {
        let rows = sqlx::query_as::<sqlx::Postgres, ProfitCenter>(
            r#"SELECT id, code, name, department_id, is_active, operator_id, created_at, updated_at, deleted_at
               FROM profit_centers
               WHERE deleted_at IS NULL AND is_active = TRUE
               ORDER BY id"#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<ProfitCenter>> {
        let row = sqlx::query_as::<sqlx::Postgres, ProfitCenter>(
            r#"SELECT id, code, name, department_id, is_active, operator_id, created_at, updated_at, deleted_at
               FROM profit_centers WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn create(
        executor: PgExecutor<'_>,
        req: &CreateProfitCenterReq,
        operator_id: i64,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO profit_centers (code, name, department_id, operator_id)
               VALUES ($1, $2, $3, $4)
               RETURNING id"#,
        )
        .bind(&req.code)
        .bind(&req.name)
        .bind(req.department_id)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateProfitCenterReq,
        operator_id: i64,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.name.is_some() {
            sets.push(format!("name = ${param_idx}"));
            param_idx += 1;
        }
        if req.department_id.is_some() {
            sets.push(format!("department_id = ${param_idx}"));
            param_idx += 1;
        }
        if req.is_active.is_some() {
            sets.push(format!("is_active = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        sets.push(format!("operator_id = ${param_idx}"));
        let sql = format!(
            "UPDATE profit_centers SET {} WHERE id = $1 AND deleted_at IS NULL",
            sets.join(", ")
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(ref v) = req.name {
            q = q.bind(v);
        }
        if let Some(v) = req.department_id {
            q = q.bind(v);
        }
        if let Some(v) = req.is_active {
            q = q.bind(v);
        }
        q = q.bind(operator_id);

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE profit_centers SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }
}
