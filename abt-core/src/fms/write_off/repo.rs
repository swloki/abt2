use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::types::PageParams;
use super::super::enums::WriteOffType;

const WRITE_OFF_COLUMNS: &str = "id, write_off_type, cash_journal_id, source_type, source_id, amount, write_off_date, idempotency_key, operator_id, created_at";

// ---------------------------------------------------------------------------
// WriteOffRepo
// ---------------------------------------------------------------------------

pub struct WriteOffRepo;

impl WriteOffRepo {
    /// Insert a write-off record. Returns the generated id.
    /// Callers rely on the unique index on idempotency_key for dedup.
    pub async fn create(
        executor: PgExecutor<'_>,
        write_off_type: WriteOffType,
        req: &WriteOffReq,
        write_off_date: chrono::NaiveDate,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO write_offs
               (write_off_type, cash_journal_id, source_type, source_id, amount, write_off_date, idempotency_key, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id"#,
        )
        .bind(write_off_type)
        .bind(req.cash_journal_id)
        .bind(req.source_type)
        .bind(req.source_id)
        .bind(req.amount)
        .bind(write_off_date)
        .bind(&req.idempotency_key)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    /// List write-offs by source document with pagination.
    /// Returns (items, total_count).
    pub async fn list_by_source(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
        page: &PageParams,
    ) -> Result<(Vec<WriteOff>, u64)> {
        // Count
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM write_offs WHERE source_type = $1 AND source_id = $2"#,
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_one(&mut *executor)
        .await?;

        // Data
        let items = sqlx::query_as::<sqlx::Postgres, WriteOff>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {WRITE_OFF_COLUMNS} FROM write_offs \
                 WHERE source_type = $1 AND source_id = $2 \
                 ORDER BY id DESC LIMIT $3 OFFSET $4"
            )),
        )
        .bind(source_type)
        .bind(source_id)
        .bind(page.page_size as i64)
        .bind(page.offset() as i64)
        .fetch_all(&mut *executor)
        .await?;

        Ok((items, total as u64))
    }

    /// List all write-offs with optional type filter and pagination.
    /// Returns (items, total_count).
    pub async fn list(
        executor: PgExecutor<'_>,
        filter: &super::model::WriteOffListFilter,
        page: &PageParams,
    ) -> Result<(Vec<WriteOff>, u64)> {
        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx: i32 = 0;
        let needs_journal_join = filter.keyword.as_ref().is_some_and(|k| !k.is_empty());

        if filter.write_off_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.write_off_type = ${param_idx}"));
        }
        if let Some(ref kw) = filter.keyword
            && !kw.is_empty() {
                param_idx += 1;
                where_clauses.push(format!("cj.doc_number ILIKE ${param_idx}"));
            }
        if filter.start_date.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.write_off_date >= ${param_idx}"));
        }
        if filter.end_date.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.write_off_date <= ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let from_sql = if needs_journal_join {
            "write_offs wo LEFT JOIN cash_journals cj ON cj.id = wo.cash_journal_id"
        } else {
            "write_offs wo"
        };

        // Count
        let count_sql = format!("SELECT COUNT(*) FROM {from_sql} WHERE {where_sql}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(wt) = filter.write_off_type {
            count_q = count_q.bind(wt);
        }
        if let Some(ref kw) = filter.keyword
            && !kw.is_empty() {
                count_q = count_q.bind(format!("%{kw}%"));
            }
        if let Some(d) = filter.start_date {
            count_q = count_q.bind(d);
        }
        if let Some(d) = filter.end_date {
            count_q = count_q.bind(d);
        }
        let total: i64 = count_q.fetch_one(&mut *executor).await?;

        // Data
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;
        let data_sql = format!(
            "SELECT wo.id, wo.write_off_type, wo.cash_journal_id, wo.source_type, wo.source_id, \
             wo.amount, wo.write_off_date, wo.idempotency_key, wo.operator_id, wo.created_at \
             FROM {from_sql} WHERE {where_sql} ORDER BY wo.id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, WriteOff>(sqlx::AssertSqlSafe(data_sql));
        if let Some(wt) = filter.write_off_type {
            data_q = data_q.bind(wt);
        }
        if let Some(ref kw) = filter.keyword
            && !kw.is_empty() {
                data_q = data_q.bind(format!("%{kw}%"));
            }
        if let Some(d) = filter.start_date {
            data_q = data_q.bind(d);
        }
        if let Some(d) = filter.end_date {
            data_q = data_q.bind(d);
        }
        let items = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64)
            .fetch_all(&mut *executor)
            .await?;

        Ok((items, total as u64))
    }

    /// Sum all write-off amounts for a given source document (plain read).
    pub async fn sum_written_off_by_source(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<Decimal> {
        let total: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(amount), 0) FROM write_offs WHERE source_type = $1 AND source_id = $2"#,
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_one(executor)
        .await?;
        Ok(total)
    }

    /// Acquire a session-level advisory lock keyed on (source_type, source_id),
    /// then sum all write-off amounts for that source.
    /// Uses pg_advisory_lock (session-level) which works regardless of transaction context.
    /// Caller MUST call release_advisory_lock after the operation completes.
    pub async fn lock_and_sum_written_off(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<Decimal> {
        sqlx::query("SELECT pg_advisory_lock($1, $2)")
            .bind(source_type.as_i16() as i64)
            .bind(source_id)
            .execute(&mut *executor)
            .await?;

        let total: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(amount), 0) FROM write_offs WHERE source_type = $1 AND source_id = $2"#,
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_one(&mut *executor)
        .await?;
        Ok(total)
    }

    /// Release the session-level advisory lock acquired by lock_and_sum_written_off.
    pub async fn release_advisory_lock(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<()> {
        sqlx::query("SELECT pg_advisory_unlock($1, $2)")
            .bind(source_type.as_i16() as i64)
            .bind(source_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// Sum all write-off amounts for a given cash journal.
    pub async fn sum_written_off_by_journal(
        executor: PgExecutor<'_>,
        cash_journal_id: i64,
    ) -> Result<Decimal> {
        let total: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(amount), 0) FROM write_offs WHERE cash_journal_id = $1"#,
        )
        .bind(cash_journal_id)
        .fetch_one(executor)
        .await?;
        Ok(total)
    }
}
