use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const QUOTATION_COLUMNS: &str = "id, doc_number, customer_id, contact_id, sales_rep_id, quotation_date, valid_until, status, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date";

// ---------------------------------------------------------------------------
// QuotationRepo
// ---------------------------------------------------------------------------

pub struct QuotationRepo;

impl QuotationRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        params: &CreateQuotationParams<'_>,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, valid_until, total_amount, total_cost, estimated_margin, payment_terms, delivery_terms, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING id"#,
        )
        .bind(params.doc_number)
        .bind(params.req.customer_id)
        .bind(params.req.contact_id)
        .bind(params.sales_rep_id)
        .bind(params.req.valid_until)
        .bind(params.total_amount)
        .bind(params.total_cost)
        .bind(params.estimated_margin)
        .bind(params.req.payment_terms.as_deref().unwrap_or(""))
        .bind(params.req.delivery_terms.as_deref().unwrap_or(""))
        .bind(params.req.remark.as_deref().unwrap_or(""))
        .bind(params.operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<Quotation>> {
        let quotation = sqlx::query_as::<sqlx::Postgres, Quotation>(
            sqlx::AssertSqlSafe(format!("SELECT {QUOTATION_COLUMNS} FROM quotations WHERE id = $1 AND deleted_at IS NULL")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(quotation)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateQuotationReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.customer_id.is_some() {
            sets.push(format!("customer_id = ${param_idx}"));
            param_idx += 1;
        }
        if req.contact_id.is_some() {
            sets.push(format!("contact_id = ${param_idx}"));
            param_idx += 1;
        }
        if req.sales_rep_id.is_some() {
            sets.push(format!("sales_rep_id = ${param_idx}"));
            param_idx += 1;
        }
        if req.valid_until.is_some() {
            sets.push(format!("valid_until = ${param_idx}"));
            param_idx += 1;
        }
        if req.payment_terms.is_some() {
            sets.push(format!("payment_terms = ${param_idx}"));
            param_idx += 1;
        }
        if req.delivery_terms.is_some() {
            sets.push(format!("delivery_terms = ${param_idx}"));
            param_idx += 1;
        }
        if req.remark.is_some() {
            sets.push(format!("remark = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let sql = format!(
            "UPDATE quotations SET {} WHERE id = $1 AND deleted_at IS NULL",
            sets.join(", ")
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(v) = req.customer_id {
            q = q.bind(v);
        }
        if let Some(v) = req.contact_id {
            q = q.bind(v);
        }
        if let Some(v) = req.sales_rep_id {
            q = q.bind(v);
        }
        if let Some(v) = req.valid_until {
            q = q.bind(v);
        }
        if let Some(ref v) = req.payment_terms {
            q = q.bind(v);
        }
        if let Some(ref v) = req.delivery_terms {
            q = q.bind(v);
        }
        if let Some(ref v) = req.remark {
            q = q.bind(v);
        }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn update_status(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        status: QuotationStatus,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE quotations SET status = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(status.as_i16())
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn soft_delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE quotations SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn update_amounts(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        total_amount: Decimal,
        total_cost: Decimal,
        estimated_margin: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE quotations SET total_amount = $2, total_cost = $3, estimated_margin = $4, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(total_amount)
        .bind(total_cost)
        .bind(estimated_margin)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn expire_overdue(&self, executor: PgExecutor<'_>) -> Result<i64> {
        let result = sqlx::query(
            "UPDATE quotations SET status = 5, updated_at = NOW() WHERE status = 2 AND valid_until < CURRENT_DATE AND deleted_at IS NULL",
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() as i64)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &QuotationQuery,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<PaginatedResult<Quotation>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let customer_param = if let Some(cid) = filter.customer_id {
            param_idx += 1;
            conditions.push(format!("customer_id = ${param_idx}"));
            Some(cid)
        } else {
            None
        };

        let status_param = if let Some(status) = filter.status {
            param_idx += 1;
            conditions.push(format!("status = ${param_idx}"));
            Some(status.as_i16())
        } else {
            None
        };

        let date_from_param = if let Some(date_from) = filter.date_from {
            param_idx += 1;
            conditions.push(format!("quotation_date >= ${param_idx}"));
            Some(date_from)
        } else {
            None
        };

        let date_to_param = if let Some(date_to) = filter.date_to {
            param_idx += 1;
            conditions.push(format!("quotation_date <= ${param_idx}"));
            Some(date_to)
        } else {
            None
        };

        let keyword_param = if let Some(ref keyword) = filter.keyword {
            param_idx += 1;
            conditions.push(format!("doc_number ILIKE ${param_idx}"));
            Some(format!("%{keyword}%"))
        } else {
            None
        };

        let scope_param = match data_scope {
            DataScope::All => None,
            DataScope::Department | DataScope::SelfOnly => {
                param_idx += 1;
                conditions.push(format!("sales_rep_id = ${param_idx}"));
                Some(scope_operator_id)
            }
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM quotations WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = customer_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = status_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = date_from_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = date_to_param {
            count_q = count_q.bind(v);
        }
        if let Some(ref v) = keyword_param {
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
            "SELECT {QUOTATION_COLUMNS} FROM quotations WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Quotation>(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = customer_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = status_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = date_from_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = date_to_param {
            data_q = data_q.bind(v);
        }
        if let Some(ref v) = keyword_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = scope_param {
            data_q = data_q.bind(v);
        }
        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}

// ---------------------------------------------------------------------------
// QuotationItemRepo
// ---------------------------------------------------------------------------

pub struct QuotationItemRepo;

impl QuotationItemRepo {
    pub async fn create_batch(
        &self,
        executor: PgExecutor<'_>,
        quotation_id: i64,
        items: &[QuotationItemInput],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO quotation_items (quotation_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
            )
            .bind(quotation_id)
            .bind(item.line_no)
            .bind(item.product_id)
            .bind(&item.description)
            .bind(item.quantity)
            .bind(&item.unit)
            .bind(item.unit_price)
            .bind(item.unit_cost)
            .bind(item.discount_rate)
            .bind(item.amount)
            .bind(item.delivery_date)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_quotation_id(
        &self,
        executor: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<Vec<QuotationItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, QuotationItem>(
            sqlx::AssertSqlSafe(format!("SELECT {ITEM_COLUMNS} FROM quotation_items WHERE quotation_id = $1 ORDER BY line_no")),
        )
        .bind(quotation_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }

    pub async fn delete_by_quotation_id(
        &self,
        executor: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM quotation_items WHERE quotation_id = $1")
            .bind(quotation_id)
            .execute(executor)
            .await?;
        Ok(())
    }
}
