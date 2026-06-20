use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use chrono::{DateTime, NaiveDate, Utc};

use super::model::*;
use super::super::enums::PeriodStatus;

const PERIOD_COLUMNS: &str = "id, name, start_date, end_date, status, fiscal_year, closed_at, closed_by, version, created_at, updated_at";

// ---------------------------------------------------------------------------
// GlPeriodRepo
// ---------------------------------------------------------------------------

pub struct GlPeriodRepo;

impl GlPeriodRepo {
    pub async fn list(
        executor: PgExecutor<'_>,
        filter: &PeriodFilter,
    ) -> Result<Vec<AccountingPeriod>> {
        let mut conditions = vec![];
        let mut param_idx = 0u32;

        let fiscal_year_param = if let Some(ref fiscal_year) = filter.fiscal_year {
            param_idx += 1;
            conditions.push(format!("fiscal_year = ${}", param_idx));
            Some(fiscal_year.clone())
        } else {
            None
        };

        let status_param = if let Some(status) = filter.status {
            param_idx += 1;
            conditions.push(format!("status = ${}", param_idx));
            Some(status)
        } else {
            None
        };

        let where_clause = if conditions.is_empty() {
            String::from("1=1")
        } else {
            conditions.join(" AND ")
        };

        let sql = format!(
            "SELECT {PERIOD_COLUMNS} FROM accounting_periods WHERE {where_clause} ORDER BY start_date ASC"
        );
        let mut query = sqlx::query_as::<sqlx::Postgres, AccountingPeriod>(sqlx::AssertSqlSafe(sql));

        if let Some(fy) = fiscal_year_param {
            query = query.bind(fy);
        }
        if let Some(s) = status_param {
            query = query.bind(s);
        }

        let periods = query.fetch_all(executor).await?;
        Ok(periods)
    }

    /// 根据日期查找期间（WHERE start_date <= d AND end_date >= d）
    pub async fn get_by_date(
        executor: PgExecutor<'_>,
        date: NaiveDate,
    ) -> Result<Option<AccountingPeriod>> {
        let period = sqlx::query_as::<sqlx::Postgres, AccountingPeriod>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {PERIOD_COLUMNS} FROM accounting_periods WHERE start_date <= $1 AND end_date >= $1"
            )),
        )
        .bind(date)
        .fetch_optional(executor)
        .await?;
        Ok(period)
    }

    /// 乐观锁更新期间状态（含 version 检查）
    /// 返回影响的行数（0 表示版本冲突）
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: PeriodStatus,
        version: i32,
        closed_at: Option<DateTime<Utc>>,
        closed_by: Option<i64>,
    ) -> Result<u64> {
        let sql = r#"
            UPDATE accounting_periods
            SET status = $1, closed_at = $2, closed_by = $3, version = version + 1, updated_at = NOW()
            WHERE id = $4 AND version = $5
        "#;

        let result = sqlx::query::<sqlx::Postgres>(sql)
            .bind(status)
            .bind(closed_at)
            .bind(closed_by)
            .bind(id)
            .bind(version)
            .execute(executor)
            .await?;

        Ok(result.rows_affected())
    }
}
