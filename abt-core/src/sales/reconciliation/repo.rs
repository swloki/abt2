use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const REC_COLUMNS: &str = "id, doc_number, customer_id, period, status, total_amount, confirmed_amount, difference, remark, operator_id, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed, remark";

// ---------------------------------------------------------------------------
// ReconciliationRepo
// ---------------------------------------------------------------------------

pub struct ReconciliationRepo;

impl ReconciliationRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        params: &CreateReconciliationParams<'_>,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO reconciliations (doc_number, customer_id, period, total_amount, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#,
        )
        .bind(params.doc_number)
        .bind(params.customer_id)
        .bind(params.period)
        .bind(params.total_amount)
        .bind(params.remark)
        .bind(params.operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<Reconciliation>> {
        let rec = sqlx::query_as::<sqlx::Postgres, Reconciliation>(
            sqlx::AssertSqlSafe(format!("SELECT {REC_COLUMNS} FROM reconciliations WHERE id = $1 AND deleted_at IS NULL")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(rec)
    }

    pub async fn exists_by_customer_period(
        &self,
        executor: PgExecutor<'_>,
        customer_id: i64,
        period: &str,
    ) -> Result<bool> {
        let count = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT COUNT(*) FROM reconciliations WHERE customer_id = $1 AND period = $2 AND deleted_at IS NULL",
        )
        .bind(customer_id)
        .bind(period)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }

    pub async fn update_status(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        status: ReconciliationStatus,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE reconciliations SET status = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(status.as_i16())
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_amounts(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        confirmed_amount: rust_decimal::Decimal,
        difference: rust_decimal::Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE reconciliations SET confirmed_amount = $2, difference = $3, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(confirmed_amount)
        .bind(difference)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn soft_delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE reconciliations SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &ReconciliationQuery,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<PaginatedResult<Reconciliation>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let customer_param = if let Some(cid) = filter.customer_id {
            param_idx += 1;
            conditions.push(format!("customer_id = ${param_idx}"));
            Some(cid)
        } else {
            None
        };

        let period_param = if let Some(ref period) = filter.period {
            param_idx += 1;
            conditions.push(format!("period = ${param_idx}"));
            Some(period.clone())
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
                conditions.push(format!("operator_id = ${param_idx}"));
                Some(scope_operator_id)
            }
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM reconciliations WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = customer_param { count_q = count_q.bind(v); }
        if let Some(ref v) = period_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        if let Some(v) = scope_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {REC_COLUMNS} FROM reconciliations WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Reconciliation>(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = customer_param { data_q = data_q.bind(v); }
        if let Some(ref v) = period_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(ref v) = keyword_param { data_q = data_q.bind(v); }
        if let Some(v) = scope_param { data_q = data_q.bind(v); }
        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}

// ---------------------------------------------------------------------------
// ReconciliationItemRepo
// ---------------------------------------------------------------------------

pub struct ReconciliationItemRepo;

impl ReconciliationItemRepo {
    pub async fn create_batch(
        &self,
        executor: PgExecutor<'_>,
        reconciliation_id: i64,
        items: &[ReconciliationItemInput],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO reconciliation_items (reconciliation_id, shipping_request_id, sales_order_id, product_id, quantity, unit_price, amount, confirmed)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(reconciliation_id)
            .bind(item.shipping_request_id)
            .bind(item.sales_order_id)
            .bind(item.product_id)
            .bind(item.quantity)
            .bind(item.unit_price)
            .bind(item.amount)
            .bind(false)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_reconciliation_id(
        &self,
        executor: PgExecutor<'_>,
        reconciliation_id: i64,
    ) -> Result<Vec<ReconciliationItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, ReconciliationItem>(
            sqlx::AssertSqlSafe(format!("SELECT {ITEM_COLUMNS} FROM reconciliation_items WHERE reconciliation_id = $1 ORDER BY id")),
        )
        .bind(reconciliation_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }

    pub async fn all_confirmed(
        &self,
        executor: PgExecutor<'_>,
        reconciliation_id: i64,
    ) -> Result<bool> {
        let unconfirmed = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT COUNT(*) FROM reconciliation_items WHERE reconciliation_id = $1 AND confirmed = FALSE",
        )
        .bind(reconciliation_id)
        .fetch_one(executor)
        .await?;
        Ok(unconfirmed == 0)
    }
}

// ---------------------------------------------------------------------------
// Aggregation query: aggregate shipping items for a customer+period
// ---------------------------------------------------------------------------

/// 聚合结果行（从 shipping_requests + shipping_request_items + sales_order_items JOIN 得到）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AggregatedShippingItem {
    pub shipping_request_id: i64,
    pub sales_order_id: i64,
    pub product_id: i64,
    pub quantity: rust_decimal::Decimal,
    pub unit_price: rust_decimal::Decimal,
    pub amount: rust_decimal::Decimal,
}

pub async fn aggregate_shipping_items(
    executor: PgExecutor<'_>,
    customer_id: i64,
    period: &str,
) -> Result<Vec<AggregatedShippingItem>> {
    let items = sqlx::query_as::<sqlx::Postgres, AggregatedShippingItem>(
        r#"SELECT
            sr.id AS shipping_request_id,
            so.id AS sales_order_id,
            sri.product_id,
            sri.shipped_qty AS quantity,
            soi.unit_price,
            (sri.shipped_qty * soi.unit_price) AS amount
        FROM shipping_requests sr
        JOIN sales_orders so ON sr.order_id = so.id
        JOIN shipping_request_items sri ON sri.shipping_request_id = sr.id
        JOIN sales_order_items soi ON soi.id = sri.order_item_id
        WHERE so.customer_id = $1
          AND sr.status = 4
          AND to_char(sr.created_at, 'YYYY-MM') = $2
          AND sr.deleted_at IS NULL
        ORDER BY sr.id, sri.line_no"#,
    )
    .bind(customer_id)
    .bind(period)
    .fetch_all(executor)
    .await?;
    Ok(items)
}
