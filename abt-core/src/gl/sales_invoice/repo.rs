use chrono::Datelike;
use rust_decimal::Decimal;

use super::model::*;
use super::super::invoice::InvoiceStatus;
use crate::shared::types::{DataScope, PageParams, PgExecutor, Result};

const INVOICE_COLUMNS: &str = "id, doc_number, customer_id, issue_date, period, subtotal, tax_amount, total, status, source_shipping_id, gl_entry_id, operator_id, version, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, invoice_id, product_id, qty, unit_price, tax_rate_id, line_subtotal, line_tax, line_total";

// ---------------------------------------------------------------------------
// SalesInvoiceRepo
// ---------------------------------------------------------------------------

pub struct SalesInvoiceRepo;

impl SalesInvoiceRepo {
    pub async fn create(
        executor: PgExecutor<'_>,
        doc_number: &str,
        req: &CreateSalesInvoiceReq,
        subtotal: Decimal,
        tax_amount: Decimal,
        total: Decimal,
        operator_id: i64,
    ) -> Result<i64> {
        let period = format!("{}-{:02}", req.issue_date.year(), req.issue_date.month());

        let id: i64 = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO sales_invoices
               (doc_number, customer_id, issue_date, period, subtotal, tax_amount, total, status, source_shipping_id, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(req.customer_id)
        .bind(req.issue_date)
        .bind(&period)
        .bind(subtotal)
        .bind(tax_amount)
        .bind(total)
        .bind(InvoiceStatus::Draft)
        .bind(req.source_shipping_id)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn batch_items(
        executor: PgExecutor<'_>,
        invoice_id: i64,
        items: &[SalesInvoiceItemInput],
    ) -> Result<u64> {
        if items.is_empty() {
            return Ok(0);
        }

        use sqlx::QueryBuilder;

        let mut query_builder = QueryBuilder::new(
            "INSERT INTO sales_invoice_items (invoice_id, product_id, qty, unit_price, tax_rate_id, line_subtotal, line_tax, line_total) VALUES "
        );

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                query_builder.push(", ");
            }
            let line_subtotal = item.qty * item.unit_price;
            // TODO: 根据税率计算税额（暂时为0）
            let line_tax = Decimal::ZERO;
            let line_total = line_subtotal + line_tax;

            query_builder.push("(");
            query_builder.push_bind(invoice_id);
            query_builder.push(", ");
            query_builder.push_bind(item.product_id);
            query_builder.push(", ");
            query_builder.push_bind(item.qty);
            query_builder.push(", ");
            query_builder.push_bind(item.unit_price);
            query_builder.push(", ");
            query_builder.push_bind(item.tax_rate_id);
            query_builder.push(", ");
            query_builder.push_bind(line_subtotal);
            query_builder.push(", ");
            query_builder.push_bind(line_tax);
            query_builder.push(", ");
            query_builder.push_bind(line_total);
            query_builder.push(")");
        }

        let result = query_builder.build().execute(executor).await?;

        Ok(result.rows_affected())
    }

    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<SalesInvoice>> {
        let invoice = sqlx::query_as::<sqlx::Postgres, SalesInvoice>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {INVOICE_COLUMNS} FROM sales_invoices WHERE id = $1 AND deleted_at IS NULL"
            )),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(invoice)
    }

    pub async fn list_items(executor: PgExecutor<'_>, invoice_id: i64) -> Result<Vec<SalesInvoiceItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, SalesInvoiceItem>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {ITEM_COLUMNS} FROM sales_invoice_items WHERE invoice_id = $1"
            )),
        )
        .bind(invoice_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }

    /// Update status with optimistic lock (version check). Returns rows affected.
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: InvoiceStatus,
        version: i32,
    ) -> Result<u64> {
        let result = sqlx::query::<sqlx::Postgres>(
            r#"UPDATE sales_invoices
               SET status = $1, version = version + 1, updated_at = NOW()
               WHERE id = $2 AND version = $3 AND deleted_at IS NULL"#,
        )
        .bind(status)
        .bind(id)
        .bind(version)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &SalesInvoiceFilter,
        page: &PageParams,
        _data_scope: DataScope,
        _scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<(Vec<SalesInvoice>, u64)> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let customer_param = if let Some(customer_id) = filter.customer_id {
            param_idx += 1;
            conditions.push(format!("customer_id = ${}", param_idx));
            Some(customer_id)
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

        let period_param = if let Some(ref period) = filter.period {
            if !period.trim().is_empty() {
                param_idx += 1;
                conditions.push(format!("period = ${}", param_idx));
                Some(period.clone())
            } else {
                None
            }
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM sales_invoices WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));

        if let Some(cid) = customer_param {
            count_q = count_q.bind(cid);
        }
        if let Some(s) = status_param {
            count_q = count_q.bind(s);
        }
        if let Some(ref p) = period_param {
            count_q = count_q.bind(p);
        }

        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data query
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {INVOICE_COLUMNS} FROM sales_invoices WHERE {where_clause} ORDER BY id DESC LIMIT ${} OFFSET ${}",
            limit_idx, offset_idx
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, SalesInvoice>(sqlx::AssertSqlSafe(data_sql));

        if let Some(cid) = customer_param {
            data_q = data_q.bind(cid);
        }
        if let Some(s) = status_param {
            data_q = data_q.bind(s);
        }
        if let Some(ref p) = period_param {
            data_q = data_q.bind(p);
        }

        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);

        let items = data_q.fetch_all(executor).await?;

        Ok((items, total))
    }

    pub async fn attach_gl_entry(
        executor: PgExecutor<'_>,
        id: i64,
        gl_entry_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query::<sqlx::Postgres>(
            r#"UPDATE sales_invoices
               SET gl_entry_id = $1, updated_at = NOW()
               WHERE id = $2 AND deleted_at IS NULL"#,
        )
        .bind(gl_entry_id)
        .bind(id)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }
}
