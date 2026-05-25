use anyhow::Result;
use common::PgExecutor;
use rust_decimal::Decimal;

use super::model::*;
use super::super::enums::JournalStatus;
use crate::shared::types::{DataScope, PageParams};

const JOURNAL_COLUMNS: &str = "id, doc_number, journal_type, direction, amount, counterparty_type, counterparty_id, source_type, source_id, bank_account, transaction_date, period, status, remark, operator_id, version, created_at, updated_at, deleted_at";

const LINE_COLUMNS: &str = "id, journal_id, account_code, debit_amount, credit_amount, cost_center, profit_center, remark";

// ---------------------------------------------------------------------------
// CashJournalRepo
// ---------------------------------------------------------------------------

pub struct CashJournalRepo;

impl CashJournalRepo {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        executor: PgExecutor<'_>,
        doc_number: &str,
        req: &CreateCashJournalReq,
        operator_id: i64,
    ) -> Result<i64> {
        let (cp_type, cp_id) = req.counterparty.to_parts();
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO cash_journals
               (doc_number, journal_type, direction, amount, counterparty_type, counterparty_id,
                source_type, source_id, bank_account, transaction_date, period, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(req.journal_type)
        .bind(req.direction)
        .bind(req.amount)
        .bind(cp_type)
        .bind(cp_id)
        .bind(req.source_type)
        .bind(req.source_id)
        .bind(&req.bank_account)
        .bind(req.transaction_date)
        .bind(&req.period)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<CashJournal>> {
        let journal = sqlx::query_as::<sqlx::Postgres, CashJournal>(
            &format!(
                "SELECT {JOURNAL_COLUMNS} FROM cash_journals WHERE id = $1 AND deleted_at IS NULL"
            ),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(journal)
    }

    /// Lock row for update (used in confirm flow)
    pub async fn get_for_update(executor: PgExecutor<'_>, id: i64) -> Result<Option<CashJournal>> {
        let journal = sqlx::query_as::<sqlx::Postgres, CashJournal>(
            &format!(
                "SELECT {JOURNAL_COLUMNS} FROM cash_journals WHERE id = $1 AND deleted_at IS NULL FOR UPDATE"
            ),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(journal)
    }

    /// Update status with optimistic lock (version check). Returns rows affected.
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: JournalStatus,
        version: i32,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE cash_journals SET status = $2, version = version + 1, updated_at = NOW() \
             WHERE id = $1 AND version = $3 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(status)
        .bind(version)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &CashJournalFilter,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<(Vec<CashJournal>, u64)> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        let period_param = if let Some(ref period) = filter.period {
            param_idx += 1;
            conditions.push(format!("period = ${param_idx}"));
            Some(period.clone())
        } else {
            None
        };

        let journal_type_param = if let Some(jt) = filter.journal_type {
            param_idx += 1;
            conditions.push(format!("journal_type = ${param_idx}"));
            Some(jt)
        } else {
            None
        };

        let status_param = if !filter.status.is_empty() {
            param_idx += 1;
            let placeholders: Vec<String> = filter
                .status
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let idx = param_idx + i as u32;
                    format!("${idx}")
                })
                .collect();
            // we will bind each status individually, so track the final param_idx
            let count = filter.status.len() as u32;
            conditions.push(format!("status IN ({})", placeholders.join(", ")));
            param_idx += count - 1; // already incremented once above
            Some(filter.status.clone())
        } else {
            None
        };

        let counterparty_param = if let Some(cp_id) = filter.counterparty_id {
            param_idx += 1;
            conditions.push(format!("counterparty_id = ${param_idx}"));
            Some(cp_id)
        } else {
            None
        };

        let date_from_param = if let Some(date_from) = filter.transaction_date_from {
            param_idx += 1;
            conditions.push(format!("transaction_date >= ${param_idx}"));
            Some(date_from)
        } else {
            None
        };

        let date_to_param = if let Some(date_to) = filter.transaction_date_to {
            param_idx += 1;
            conditions.push(format!("transaction_date <= ${param_idx}"));
            Some(date_to)
        } else {
            None
        };

        let scope_param = match data_scope {
            DataScope::All => None,
            // cash_journals has no department_id column; Department falls back to operator_id
            DataScope::Department | DataScope::SelfOnly => {
                param_idx += 1;
                conditions.push(format!("operator_id = ${param_idx}"));
                Some(scope_operator_id)
            }
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM cash_journals WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(ref v) = period_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = journal_type_param {
            count_q = count_q.bind(v);
        }
        if let Some(ref statuses) = status_param {
            for s in statuses {
                count_q = count_q.bind(s);
            }
        }
        if let Some(v) = counterparty_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = date_from_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = date_to_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = scope_param {
            count_q = count_q.bind(v);
        }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data query
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {JOURNAL_COLUMNS} FROM cash_journals WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, CashJournal>(&data_sql);
        if let Some(ref v) = period_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = journal_type_param {
            data_q = data_q.bind(v);
        }
        if let Some(ref statuses) = status_param {
            for s in statuses {
                data_q = data_q.bind(s);
            }
        }
        if let Some(v) = counterparty_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = date_from_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = date_to_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = scope_param {
            data_q = data_q.bind(v);
        }
        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok((items, total))
    }

    /// Sum inflow (direction=1) and outflow (direction=2) for a period.
    /// Returns (total_inflow, total_outflow).
    pub async fn sum_balance_by_period(
        executor: PgExecutor<'_>,
        period: &str,
    ) -> Result<(Decimal, Decimal)> {
        let row: (Decimal, Decimal) = sqlx::query_as(
            r#"SELECT
                 COALESCE(SUM(amount) FILTER (WHERE direction = 1), 0),
                 COALESCE(SUM(amount) FILTER (WHERE direction = 2), 0)
               FROM cash_journals
               WHERE period = $1 AND status = 2 AND deleted_at IS NULL"#,
        )
        .bind(period)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// CashJournalLineRepo
// ---------------------------------------------------------------------------

pub struct CashJournalLineRepo;

impl CashJournalLineRepo {
    pub async fn batch_insert(
        executor: PgExecutor<'_>,
        journal_id: i64,
        lines: &[CashJournalLineInput],
    ) -> Result<()> {
        for line in lines {
            sqlx::query(
                r#"INSERT INTO cash_journal_lines
                   (journal_id, account_code, debit_amount, credit_amount, cost_center, profit_center, remark)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(journal_id)
            .bind(&line.account_code)
            .bind(line.debit_amount)
            .bind(line.credit_amount)
            .bind(line.cost_center)
            .bind(line.profit_center)
            .bind(&line.remark)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn get_by_journal_id(
        executor: PgExecutor<'_>,
        journal_id: i64,
    ) -> Result<Vec<CashJournalLine>> {
        let lines = sqlx::query_as::<sqlx::Postgres, CashJournalLine>(
            &format!("SELECT {LINE_COLUMNS} FROM cash_journal_lines WHERE journal_id = $1"),
        )
        .bind(journal_id)
        .fetch_all(executor)
        .await?;
        Ok(lines)
    }

    /// Sum total debit and credit for a journal. Returns (total_debit, total_credit).
    pub async fn sum_debit_credit(
        executor: PgExecutor<'_>,
        journal_id: i64,
    ) -> Result<(Decimal, Decimal)> {
        let row: (Decimal, Decimal) = sqlx::query_as(
            r#"SELECT
                 COALESCE(SUM(debit_amount), 0),
                 COALESCE(SUM(credit_amount), 0)
               FROM cash_journal_lines
               WHERE journal_id = $1"#,
        )
        .bind(journal_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }
}
