use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use rust_decimal::Decimal;
use chrono::NaiveDate;
use sqlx::Row;

use super::model::*;
use crate::shared::types::{DataScope, PageParams};
use super::super::enums::EntryStatus;

const ENTRY_COLUMNS: &str = "id, doc_number, period, entry_date, source_type, source_id, description, voucher_type, is_opening, status, total_debit, total_credit, operator_id, version, created_at, updated_at, deleted_at";

const LINE_COLUMNS: &str = "id, entry_id, account_id, debit, credit, amount_currency, currency, exchange_rate, cost_center, profit_center, project_id, memo";

// ---------------------------------------------------------------------------
// GlEntryRepo
// ---------------------------------------------------------------------------

pub struct GlEntryRepo;

impl GlEntryRepo {
    /// 创建凭证头（返回 id）
    pub async fn create_entry(
        executor: PgExecutor<'_>,
        doc_number: &str,
        req: &CreateManualEntryReq,
        period: &str,
        source_type: crate::shared::enums::document_type::DocumentType,
        operator_id: i64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO gl_entries (doc_number, period, entry_date, source_type, source_id, description, voucher_type, is_opening, status, operator_id)
               VALUES ($1, $2, $3, $4, 0, $5, $6, $7, 1, $8)
               RETURNING id"#
        )
        .bind(doc_number)
        .bind(period)
        .bind(req.entry_date)
        .bind(source_type)
        .bind(&req.description)
        .bind(&req.voucher_type)
        .bind(req.is_opening)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 批量插入分录行
    pub async fn batch_lines(
        executor: PgExecutor<'_>,
        entry_id: i64,
        lines: &[GlEntryLineInput],
    ) -> Result<()> {
        for line in lines {
            sqlx::query::<sqlx::Postgres>(
                r#"INSERT INTO gl_entry_lines (entry_id, account_id, debit, credit, amount_currency, currency, exchange_rate, cost_center, profit_center, project_id, memo)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#
            )
            .bind(entry_id)
            .bind(line.account_id)
            .bind(line.debit)
            .bind(line.credit)
            .bind(Decimal::ZERO) // amount_currency 本期默认 0
            .bind("CNY")         // currency 本期默认 CNY
            .bind(Decimal::ONE)  // exchange_rate 本期默认 1
            .bind(line.cost_center)
            .bind(line.profit_center)
            .bind(line.project_id)
            .bind(&line.memo)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 获取凭证头（用于 get）
    pub async fn get_entry(executor: PgExecutor<'_>, id: i64) -> Result<Option<GlEntry>> {
        let entry = sqlx::query_as::<sqlx::Postgres, GlEntry>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {ENTRY_COLUMNS} FROM gl_entries WHERE id = $1 AND deleted_at IS NULL"
            )),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(entry)
    }

    /// 获取凭证所有行（用于 get）
    pub async fn list_lines(executor: PgExecutor<'_>, entry_id: i64) -> Result<Vec<GlEntryLine>> {
        let lines = sqlx::query_as::<sqlx::Postgres, GlEntryLine>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {LINE_COLUMNS} FROM gl_entry_lines WHERE entry_id = $1"
            )),
        )
        .bind(entry_id)
        .fetch_all(executor)
        .await?;
        Ok(lines)
    }

    /// 更新凭证状态 + 金额（乐观锁，返回 rows affected）
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: EntryStatus,
        total_debit: Decimal,
        total_credit: Decimal,
        version: i32,
    ) -> Result<u64> {
        let result = sqlx::query::<sqlx::Postgres>(
            r#"UPDATE gl_entries
               SET status = $1, total_debit = $2, total_credit = $3, version = version + 1, updated_at = NOW()
               WHERE id = $4 AND version = $5 AND deleted_at IS NULL"#
        )
        .bind(status)
        .bind(total_debit)
        .bind(total_credit)
        .bind(id)
        .bind(version)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 查询凭证列表（支持分页和过滤）
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &GlEntryFilter,
        page: &PageParams,
        _data_scope: DataScope,
        _scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<(Vec<GlEntry>, u64)> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let period_param = if let Some(ref period) = filter.period {
            param_idx += 1;
            conditions.push(format!("period = ${}", param_idx));
            Some(period.clone())
        } else {
            None
        };

        let source_type_param = if let Some(source_type) = filter.source_type {
            param_idx += 1;
            conditions.push(format!("source_type = ${}", param_idx));
            Some(source_type)
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

        let voucher_type_param = if let Some(ref voucher_type) = filter.voucher_type {
            param_idx += 1;
            conditions.push(format!("voucher_type = ${}", param_idx));
            Some(voucher_type.clone())
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM gl_entries WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));

        if let Some(ref p) = period_param {
            count_q = count_q.bind(p);
        }
        if let Some(st) = source_type_param {
            count_q = count_q.bind(st);
        }
        if let Some(s) = status_param {
            count_q = count_q.bind(s);
        }
        if let Some(ref vt) = voucher_type_param {
            count_q = count_q.bind(vt);
        }

        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data query
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {ENTRY_COLUMNS} FROM gl_entries WHERE {where_clause} ORDER BY entry_date DESC, id DESC LIMIT ${} OFFSET ${}",
            limit_idx, offset_idx
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, GlEntry>(sqlx::AssertSqlSafe(data_sql));

        if let Some(ref p) = period_param {
            data_q = data_q.bind(p);
        }
        if let Some(st) = source_type_param {
            data_q = data_q.bind(st);
        }
        if let Some(s) = status_param {
            data_q = data_q.bind(s);
        }
        if let Some(ref vt) = voucher_type_param {
            data_q = data_q.bind(vt);
        }

        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);

        let items = data_q.fetch_all(executor).await?;

        Ok((items, total))
    }

    /// 按期间汇总分录（用于试算平衡）- 只统计 posted 凭证
    pub async fn sum_lines_by_period(executor: PgExecutor<'_>, period: &str) -> Result<Vec<TrialBalanceRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, TrialBalanceRow>(
            r#"
            SELECT a.id AS account_id,
                   a.code,
                   a.name,
                   a.account_type,
                   a.balance_direction,
                   COALESCE(SUM(l.debit), 0) AS period_debit,
                   COALESCE(SUM(l.credit), 0) AS period_credit,
                   0 AS end_balance  -- 将在 service 层计算
            FROM gl_accounts a
            LEFT JOIN gl_entry_lines l ON l.account_id = a.id
            LEFT JOIN gl_entries e ON e.id = l.entry_id
                AND e.period = $1
                AND e.status = 2  -- Posted
                AND e.deleted_at IS NULL
            WHERE a.deleted_at IS NULL
            GROUP BY a.id, a.code, a.name, a.account_type, a.balance_direction
            ORDER BY a.code
            "#
        )
        .bind(period)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 获取科目的所有 posted 分录（用于总账明细账）
    pub async fn get_posted_lines_by_account(
        executor: PgExecutor<'_>,
        account_id: i64,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<(GlEntry, GlEntryLine)>> {
        let mut conditions = vec![
            "e.deleted_at IS NULL".to_string(),
            "e.status = 2".to_string(),  // Posted
            "l.account_id = $1".to_string(),
        ];
        let mut param_idx = 1;

        if from.is_some() {
            param_idx += 1;
            conditions.push(format!("e.entry_date >= ${}", param_idx));
        }

        if to.is_some() {
            param_idx += 1;
            conditions.push(format!("e.entry_date <= ${}", param_idx));
        }

        let where_clause = conditions.join(" AND ");

        let sql = format!(
            r#"SELECT {ENTRY_COLUMNS}, {LINE_COLUMNS}
               FROM gl_entries e
               JOIN gl_entry_lines l ON l.entry_id = e.id
               WHERE {}
               ORDER BY e.entry_date ASC, e.id ASC"#
            , where_clause
        );

        let mut query = sqlx::query::<sqlx::Postgres>(sqlx::AssertSqlSafe(sql));
        query = query.bind(account_id);

        if let Some(from_date) = from {
            query = query.bind(from_date);
        }

        if let Some(to_date) = to {
            query = query.bind(to_date);
        }

        let rows = query.fetch_all(executor).await?;

        // 手动解析 combined result
        let mut results = Vec::new();
        for row in rows {
            let entry: GlEntry = GlEntry {
                id: row.try_get("id")?,
                doc_number: row.try_get("doc_number")?,
                period: row.try_get("period")?,
                entry_date: row.try_get("entry_date")?,
                source_type: row.try_get("source_type")?,
                source_id: row.try_get("source_id")?,
                description: row.try_get("description")?,
                voucher_type: row.try_get("voucher_type")?,
                is_opening: row.try_get("is_opening")?,
                status: row.try_get("status")?,
                total_debit: row.try_get("total_debit")?,
                total_credit: row.try_get("total_credit")?,
                operator_id: row.try_get("operator_id")?,
                version: row.try_get("version")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
                deleted_at: row.try_get("deleted_at")?,
            };

            let line: GlEntryLine = GlEntryLine {
                id: row.try_get("id_1")?,
                entry_id: row.try_get("entry_id")?,
                account_id: row.try_get("account_id")?,
                debit: row.try_get("debit")?,
                credit: row.try_get("credit")?,
                amount_currency: row.try_get("amount_currency")?,
                currency: row.try_get("currency")?,
                exchange_rate: row.try_get("exchange_rate")?,
                cost_center: row.try_get("cost_center")?,
                profit_center: row.try_get("profit_center")?,
                project_id: row.try_get("project_id")?,
                memo: row.try_get("memo_1")?,
            };

            results.push((entry, line));
        }

        Ok(results)
    }

    /// 获取科目的所有 posted 分录汇总（用于余额查询）
    pub async fn get_account_posted_summary(
        executor: PgExecutor<'_>,
        account_id: i64,
        period: Option<&str>,
        as_of_date: Option<NaiveDate>,
    ) -> Result<(Decimal, Decimal)> {
        let mut conditions = vec![
            "e.deleted_at IS NULL".to_string(),
            "e.status = 2".to_string(),  // Posted
            "l.account_id = $1".to_string(),
        ];
        let mut param_idx = 1;

        if period.is_some() {
            param_idx += 1;
            conditions.push(format!("e.period = ${}", param_idx));
        }

        if as_of_date.is_some() {
            param_idx += 1;
            conditions.push(format!("e.entry_date <= ${}", param_idx));
        }

        let where_clause = conditions.join(" AND ");

        let sql = format!(
            r#"SELECT COALESCE(SUM(l.debit), 0) AS total_debit,
                      COALESCE(SUM(l.credit), 0) AS total_credit
               FROM gl_entry_lines l
               JOIN gl_entries e ON e.id = l.entry_id
               WHERE {}"#, where_clause
        );

        let mut query = sqlx::query::<sqlx::Postgres>(sqlx::AssertSqlSafe(sql));
        query = query.bind(account_id);

        if let Some(p) = period {
            query = query.bind(p);
        }

        if let Some(d) = as_of_date {
            query = query.bind(d);
        }

        let row = query.fetch_one(executor).await?;
        let total_debit: Decimal = row.try_get("total_debit")?;
        let total_credit: Decimal = row.try_get("total_credit")?;

        Ok((total_debit, total_credit))
    }
}
