use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{DataScope, PageParams};

const ACCOUNT_COLUMNS: &str = "id, code, name, account_type, parent_id, is_detail, balance_direction, company_id, reconcile, disabled, opening_balance, currency, version, created_at, updated_at, deleted_at";

// ---------------------------------------------------------------------------
// GlAccountRepo
// ---------------------------------------------------------------------------

pub struct GlAccountRepo;

impl GlAccountRepo {
    pub async fn create(
        executor: PgExecutor<'_>,
        req: &CreateGlAccountReq,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO gl_accounts
               (code, name, account_type, parent_id, is_detail, balance_direction, reconcile, opening_balance, currency)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"#,
        )
        .bind(&req.code)
        .bind(&req.name)
        .bind(req.account_type)
        .bind(req.parent_id)
        .bind(req.is_detail)
        .bind(req.balance_direction)
        .bind(req.reconcile)
        .bind(req.opening_balance)
        .bind(&req.currency)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<GlAccount>> {
        let account = sqlx::query_as::<sqlx::Postgres, GlAccount>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {ACCOUNT_COLUMNS} FROM gl_accounts WHERE id = $1 AND deleted_at IS NULL"
            )),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(account)
    }

    pub async fn get_by_code(executor: PgExecutor<'_>, code: &str) -> Result<Option<GlAccount>> {
        let account = sqlx::query_as::<sqlx::Postgres, GlAccount>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {ACCOUNT_COLUMNS} FROM gl_accounts WHERE code = $1 AND deleted_at IS NULL"
            )),
        )
        .bind(code)
        .fetch_optional(executor)
        .await?;
        Ok(account)
    }

    /// Update name/disabled with optimistic lock (version check). Returns rows affected.
    pub async fn update(
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateGlAccountReq,
    ) -> Result<u64> {
        let mut set_clauses = vec![];
        let mut param_idx = 0u32;

        if req.name.is_some() {
            param_idx += 1;
            set_clauses.push(format!("name = ${}", param_idx));
        }
        if req.disabled.is_some() {
            param_idx += 1;
            set_clauses.push(format!("disabled = ${}", param_idx));
        }

        if set_clauses.is_empty() {
            return Ok(0);
        }

        param_idx += 1; // for version
        let set_clause = set_clauses.join(", ");

        let sql = format!(
            "UPDATE gl_accounts SET {set_clause}, version = version + 1, updated_at = NOW() \
             WHERE id = $1 AND version = ${} AND deleted_at IS NULL",
            param_idx + 1
        );

        let mut query = sqlx::query::<sqlx::Postgres>(sqlx::AssertSqlSafe(sql));
        query = query.bind(id);

        if let Some(ref name) = req.name {
            query = query.bind(name);
        }
        if let Some(disabled) = req.disabled {
            query = query.bind(disabled);
        }

        query = query.bind(req.version);

        let result = query.execute(executor).await?;
        Ok(result.rows_affected())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &GlAccountFilter,
        page: &PageParams,
        _data_scope: DataScope,
        _scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<(Vec<GlAccount>, u64)> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let keyword_param = if let Some(ref keyword) = filter.keyword {
            if !keyword.trim().is_empty() {
                param_idx += 1;
                conditions.push(format!("(code LIKE ${} OR name LIKE ${})", param_idx, param_idx));
                Some(format!("{}%", keyword))
            } else {
                None
            }
        } else {
            None
        };

        let account_type_param = if let Some(account_type) = filter.account_type {
            param_idx += 1;
            conditions.push(format!("account_type = ${}", param_idx));
            Some(account_type)
        } else {
            None
        };

        let disabled_param = if let Some(disabled) = filter.disabled {
            param_idx += 1;
            conditions.push(format!("disabled = ${}", param_idx));
            Some(disabled)
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM gl_accounts WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));

        if let Some(ref kw) = keyword_param {
            count_q = count_q.bind(kw).bind(kw);
        }
        if let Some(at) = account_type_param {
            count_q = count_q.bind(at);
        }
        if let Some(d) = disabled_param {
            count_q = count_q.bind(d);
        }

        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data query
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {ACCOUNT_COLUMNS} FROM gl_accounts WHERE {where_clause} ORDER BY code ASC LIMIT ${} OFFSET ${}",
            limit_idx, offset_idx
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, GlAccount>(sqlx::AssertSqlSafe(data_sql));

        if let Some(ref kw) = keyword_param {
            data_q = data_q.bind(kw).bind(kw);
        }
        if let Some(at) = account_type_param {
            data_q = data_q.bind(at);
        }
        if let Some(d) = disabled_param {
            data_q = data_q.bind(d);
        }

        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);

        let items = data_q.fetch_all(executor).await?;

        Ok((items, total))
    }
}
