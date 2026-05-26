use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use super::super::enums::ExpenseStatus;
use crate::shared::types::{DataScope, PageParams};

const EXPENSE_COLUMNS: &str = "id, doc_number, applicant_id, department_id, expense_date, total_amount, status, remark, operator_id, version, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, reimbursement_id, expense_type, amount, description, receipt_no, cost_center, profit_center";

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
               (doc_number, applicant_id, department_id, expense_date, total_amount, status, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<ExpenseReimbursement>> {
        let expense = sqlx::query_as::<sqlx::Postgres, ExpenseReimbursement>(
            &format!(
                "SELECT {EXPENSE_COLUMNS} FROM expense_reimbursements WHERE id = $1 AND deleted_at IS NULL"
            ),
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
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
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
        let mut data_q = sqlx::query_as::<sqlx::Postgres, ExpenseReimbursement>(&data_sql);
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
                   (reimbursement_id, expense_type, amount, description, receipt_no, cost_center, profit_center)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(reimbursement_id)
            .bind(item.expense_type)
            .bind(item.amount)
            .bind(&item.description)
            .bind(&item.receipt_no)
            .bind(item.cost_center)
            .bind(item.profit_center)
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
            &format!("SELECT {ITEM_COLUMNS} FROM expense_reimbursement_items WHERE reimbursement_id = $1"),
        )
        .bind(reimbursement_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }
}
