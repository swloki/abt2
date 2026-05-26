use chrono::{DateTime, Utc};
use sqlx::Row;
use crate::shared::types::RepoResult;

use super::model::{CreatePaymentRequestRequest, PaymentRequest, PaymentRequestQuery};
use crate::purchase::enums::PaymentStatus;
use crate::shared::types::pagination::PageParams;

pub struct PaymentRequestRepo;

impl PaymentRequestRepo {
    /// INSERT 付款申请，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreatePaymentRequestRequest,
        doc_number: &str,
        operator_id: i64,
    ) -> RepoResult<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO payment_requests
                (doc_number, supplier_id, reconciliation_id, payment_date, amount,
                 status, payment_method, bank_account_id, invoice_number,
                 invoice_amount, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(req.supplier_id)
        .bind(req.reconciliation_id)
        .bind(req.payment_date)
        .bind(req.amount)
        .bind(PaymentStatus::Draft)
        .bind(req.payment_method)
        .bind(req.bank_account_id)
        .bind(&req.invoice_number)
        .bind(req.invoice_amount)
        .bind(&req.remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(row.try_get("id")?)
    }

    /// 按主键查询（软删除行过滤）
    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> RepoResult<Option<PaymentRequest>> {
        sqlx::query_as::<_, PaymentRequest>(
            r#"
            SELECT id, doc_number, supplier_id, reconciliation_id, payment_date, amount,
                   status, payment_method, bank_account_id, invoice_number,
                   invoice_amount, remark, operator_id, created_at, updated_at, deleted_at
            FROM payment_requests
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    /// 动态条件分页查询
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &PaymentRequestQuery,
        page: &PageParams,
    ) -> RepoResult<(Vec<PaymentRequest>, u64)> {
        let where_clause = "
            WHERE deleted_at IS NULL
              AND ($1::bigint IS NULL OR supplier_id = $1)
              AND ($2::smallint IS NULL OR status = $2)
              AND ($3::date IS NULL OR payment_date >= $3)
              AND ($4::date IS NULL OR payment_date <= $4)
        ";

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM payment_requests {where_clause}");
        let count_row = sqlx::query(&count_sql)
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.payment_date_start)
            .bind(q.payment_date_end)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let data_sql = format!(
            "SELECT id, doc_number, supplier_id, reconciliation_id, payment_date, amount,
                    status, payment_method, bank_account_id, invoice_number,
                    invoice_amount, remark, operator_id, created_at, updated_at, deleted_at
             FROM payment_requests {where_clause}
             ORDER BY created_at DESC
             LIMIT $5 OFFSET $6"
        );
        let rows = sqlx::query_as::<_, PaymentRequest>(&data_sql)
            .bind(q.supplier_id)
            .bind(q.status)
            .bind(q.payment_date_start)
            .bind(q.payment_date_end)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        Ok((rows, total as u64))
    }

    /// 状态变更（乐观锁：WHERE updated_at = $2）
    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: PaymentStatus,
        updated_at: &DateTime<Utc>,
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE payment_requests
            SET status = $1, updated_at = NOW()
            WHERE id = $2 AND updated_at = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(status)
        .bind(id)
        .bind(updated_at)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 标记已付款（由 FMS 模块回调）
    pub async fn mark_paid(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        payment_doc_no: &str,
        updated_at: &DateTime<Utc>,
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE payment_requests
            SET status = $1, remark = remark || $2, updated_at = NOW()
            WHERE id = $3 AND updated_at = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(PaymentStatus::Paid)
        .bind(format!(" | FMS付款单号: {payment_doc_no}"))
        .bind(id)
        .bind(updated_at)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }
}
