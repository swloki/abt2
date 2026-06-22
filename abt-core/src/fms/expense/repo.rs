use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use super::super::enums::ExpenseStatus;
use crate::shared::types::{DataScope, PageParams};

const EXPENSE_COLUMNS: &str = "id, doc_number, applicant_id, department_id, expense_date, total_amount, status, remark, operator_id, version, created_at, updated_at, deleted_at, sheet_count, has_invoice, payment_remark, payment_bank, payment_date, supervisor_id";

const ITEM_COLUMNS: &str = "id, reimbursement_id, expense_type, amount, description, receipt_no, cost_center, profit_center, occurrence_date, has_invoice";

const ATTACHMENT_COLUMNS: &str = "id, expense_id, file_name, file_path, mime_type, file_size, sort_order, created_at";

// ---------------------------------------------------------------------------
// ExpenseReimbursementRepo
// ---------------------------------------------------------------------------

pub struct ExpenseReimbursementRepo;

impl ExpenseReimbursementRepo {
    pub async fn create(
        executor: PgExecutor<'_>,
        doc_number: &str,
        req: &CreateExpenseReq,
        total_amount: rust_decimal::Decimal,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO expense_reimbursements
               (doc_number, applicant_id, department_id, expense_date, total_amount, status, remark, operator_id, sheet_count, has_invoice)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(req.applicant_id)
        .bind(req.department_id)
        .bind(req.expense_date)
        .bind(total_amount)
        .bind(ExpenseStatus::Draft)
        .bind(&req.remark)
        .bind(operator_id)
        .bind(req.sheet_count)
        .bind(req.has_invoice)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<ExpenseReimbursement>> {
        let expense = sqlx::query_as::<sqlx::Postgres, ExpenseReimbursement>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {EXPENSE_COLUMNS} FROM expense_reimbursements WHERE id = $1 AND deleted_at IS NULL"
            )),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(expense)
    }

    /// Update status with optimistic lock (version check). Returns rows affected.
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: ExpenseStatus,
        version: i32,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE expense_reimbursements SET status = $2, version = version + 1, updated_at = NOW() \
             WHERE id = $1 AND version = $3 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(status)
        .bind(version)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// Update supervisor_id (called during submit)
    pub async fn update_supervisor(
        executor: PgExecutor<'_>,
        id: i64,
        supervisor_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE expense_reimbursements SET supervisor_id = $2, updated_at = NOW() \
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(supervisor_id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// Update payment info (called during pay)
    pub async fn update_payment_info(
        executor: PgExecutor<'_>,
        id: i64,
        payment_bank: &str,
        payment_remark: &str,
        payment_date: chrono::NaiveDate,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE expense_reimbursements SET payment_bank = $2, payment_remark = $3, payment_date = $4, updated_at = NOW() \
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(payment_bank)
        .bind(payment_remark)
        .bind(payment_date)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &ExpenseFilter,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<(Vec<ExpenseReimbursement>, u64)> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

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
            let count = filter.status.len() as u32;
            conditions.push(format!("status IN ({})", placeholders.join(", ")));
            param_idx += count - 1;
            Some(filter.status.clone())
        } else {
            None
        };

        let applicant_param = if let Some(applicant_id) = filter.applicant_id {
            param_idx += 1;
            conditions.push(format!("applicant_id = ${param_idx}"));
            Some(applicant_id)
        } else {
            None
        };

        let department_param = if let Some(department_id) = filter.department_id {
            param_idx += 1;
            conditions.push(format!("department_id = ${param_idx}"));
            Some(department_id)
        } else {
            None
        };

        let date_from_param = if let Some(date_from) = filter.expense_date_from {
            param_idx += 1;
            conditions.push(format!("expense_date >= ${param_idx}"));
            Some(date_from)
        } else {
            None
        };

        let date_to_param = if let Some(date_to) = filter.expense_date_to {
            param_idx += 1;
            conditions.push(format!("expense_date <= ${param_idx}"));
            Some(date_to)
        } else {
            None
        };

        let scope_param = match data_scope {
            DataScope::All => None,
            DataScope::Department => {
                if let Some(dept_id) = _scope_department_id {
                    param_idx += 1;
                    conditions.push(format!("department_id = ${param_idx}"));
                    Some(dept_id)
                } else {
                    // No department info available, fall back to operator scope
                    param_idx += 1;
                    conditions.push(format!("applicant_id = ${param_idx}"));
                    Some(scope_operator_id)
                }
            }
            DataScope::SelfOnly => {
                param_idx += 1;
                conditions.push(format!("applicant_id = ${param_idx}"));
                Some(scope_operator_id)
            }
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM expense_reimbursements WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref statuses) = status_param {
            for s in statuses {
                count_q = count_q.bind(*s);
            }
        }
        if let Some(v) = applicant_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = department_param {
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
            "SELECT {EXPENSE_COLUMNS} FROM expense_reimbursements WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, ExpenseReimbursement>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref statuses) = status_param {
            for s in statuses {
                data_q = data_q.bind(*s);
            }
        }
        if let Some(v) = applicant_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = department_param {
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

    /// Count and sum of pending (submitted + supervisor_approved + finance_approved) expense reimbursements.
    pub async fn pending_summary(executor: PgExecutor<'_>) -> Result<(i64, rust_decimal::Decimal)> {
        let row: (i64, rust_decimal::Decimal) = sqlx::query_as(
            r#"SELECT COUNT(*), COALESCE(SUM(total_amount), 0)
               FROM expense_reimbursements
               WHERE status IN (2, 6, 7) AND deleted_at IS NULL"#,
        )
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    /// Check if department has a leader
    pub async fn get_department_leader(executor: PgExecutor<'_>, department_id: i64) -> Result<Option<i64>> {
        let leader: Option<i64> = sqlx::query_scalar(
            "SELECT leader_id FROM departments WHERE department_id = $1 AND is_active = TRUE"
        )
        .bind(department_id)
        .fetch_optional(executor)
        .await?
        .flatten();
        Ok(leader)
    }
}

// ---------------------------------------------------------------------------
// ExpenseReimbursementItemRepo
// ---------------------------------------------------------------------------

pub struct ExpenseReimbursementItemRepo;

impl ExpenseReimbursementItemRepo {
    pub async fn batch_insert(
        executor: PgExecutor<'_>,
        reimbursement_id: i64,
        items: &[ExpenseItemInput],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO expense_reimbursement_items
                   (reimbursement_id, expense_type, amount, description, receipt_no, cost_center, profit_center, occurrence_date, has_invoice)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
            )
            .bind(reimbursement_id)
            .bind(item.expense_type)
            .bind(item.amount)
            .bind(&item.description)
            .bind(&item.receipt_no)
            .bind(item.cost_center)
            .bind(item.profit_center)
            .bind(item.occurrence_date)
            .bind(item.has_invoice)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn get_by_reimbursement_id(
        executor: PgExecutor<'_>,
        reimbursement_id: i64,
    ) -> Result<Vec<ExpenseReimbursementItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, ExpenseReimbursementItem>(
            sqlx::AssertSqlSafe(format!("SELECT {ITEM_COLUMNS} FROM expense_reimbursement_items WHERE reimbursement_id = $1")),
        )
        .bind(reimbursement_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }
}

// ---------------------------------------------------------------------------
// ExpenseAttachmentRepo
// ---------------------------------------------------------------------------

pub struct ExpenseAttachmentRepo;

impl ExpenseAttachmentRepo {
    pub async fn insert(
        executor: PgExecutor<'_>,
        expense_id: i64,
        req: &CreateAttachmentReq,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO expense_attachments
               (expense_id, file_name, file_path, mime_type, file_size, sort_order)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#,
        )
        .bind(expense_id)
        .bind(&req.file_name)
        .bind(&req.file_path)
        .bind(&req.mime_type)
        .bind(req.file_size)
        .bind(req.sort_order)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn list_by_expense_id(
        executor: PgExecutor<'_>,
        expense_id: i64,
    ) -> Result<Vec<ExpenseAttachment>> {
        let items = sqlx::query_as::<sqlx::Postgres, ExpenseAttachment>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {ATTACHMENT_COLUMNS} FROM expense_attachments WHERE expense_id = $1 ORDER BY sort_order ASC"
            )),
        )
        .bind(expense_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }

    pub async fn delete(executor: PgExecutor<'_>, attachment_id: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM expense_attachments WHERE id = $1")
            .bind(attachment_id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn batch_insert(
        executor: PgExecutor<'_>,
        expense_id: i64,
        attachments: &[CreateAttachmentReq],
    ) -> Result<()> {
        for att in attachments {
            Self::insert(executor, expense_id, att).await?;
        }
        Ok(())
    }
}
