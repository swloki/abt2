use crate::shared::types::PgExecutor;
use crate::shared::types::RepoResult;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const RETURN_COLUMNS: &str = "id, doc_number, order_id, shipping_request_id, customer_id, return_date, status, return_reason, total_amount, remark, operator_id, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition";

// ---------------------------------------------------------------------------
// SalesReturnRepo
// ---------------------------------------------------------------------------

pub struct SalesReturnRepo;

impl SalesReturnRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        doc_number: &str,
        order_id: i64,
        shipping_request_id: i64,
        customer_id: i64,
        return_reason: &str,
        total_amount: rust_decimal::Decimal,
        remark: &str,
        operator_id: i64,
    ) -> RepoResult<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO sales_returns (doc_number, order_id, shipping_request_id, customer_id, return_reason, total_amount, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(order_id)
        .bind(shipping_request_id)
        .bind(customer_id)
        .bind(return_reason)
        .bind(total_amount)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
    ) -> RepoResult<Option<SalesReturn>> {
        let sr = sqlx::query_as::<sqlx::Postgres, SalesReturn>(
            &format!("SELECT {RETURN_COLUMNS} FROM sales_returns WHERE id = $1 AND deleted_at IS NULL"),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(sr)
    }

    pub async fn update_status(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        status: ReturnStatus,
    ) -> RepoResult<()> {
        sqlx::query(
            "UPDATE sales_returns SET status = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(status.as_i16())
        .execute(executor)
        .await?;
        Ok(())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &ReturnQuery,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> RepoResult<PaginatedResult<SalesReturn>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let order_param = if let Some(oid) = filter.order_id {
            param_idx += 1;
            conditions.push(format!("order_id = ${param_idx}"));
            Some(oid)
        } else {
            None
        };

        let shipping_param = if let Some(sid) = filter.shipping_request_id {
            param_idx += 1;
            conditions.push(format!("shipping_request_id = ${param_idx}"));
            Some(sid)
        } else {
            None
        };

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

        let count_sql = format!("SELECT COUNT(*) FROM sales_returns WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(v) = order_param { count_q = count_q.bind(v); }
        if let Some(v) = shipping_param { count_q = count_q.bind(v); }
        if let Some(v) = customer_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        if let Some(v) = scope_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {RETURN_COLUMNS} FROM sales_returns WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, SalesReturn>(&data_sql);
        if let Some(v) = order_param { data_q = data_q.bind(v); }
        if let Some(v) = shipping_param { data_q = data_q.bind(v); }
        if let Some(v) = customer_param { data_q = data_q.bind(v); }
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
// SalesReturnItemRepo
// ---------------------------------------------------------------------------

pub struct SalesReturnItemRepo;

impl SalesReturnItemRepo {
    pub async fn create_batch(
        &self,
        executor: PgExecutor<'_>,
        return_id: i64,
        items: &[ReturnItemInput],
    ) -> RepoResult<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO sales_return_items (return_id, order_item_id, product_id, returned_qty, unit_price, amount, disposition)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(return_id)
            .bind(item.order_item_id)
            .bind(item.product_id)
            .bind(item.returned_qty)
            .bind(item.unit_price)
            .bind(item.amount)
            .bind(item.disposition.as_i16())
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_return_id(
        &self,
        executor: PgExecutor<'_>,
        return_id: i64,
    ) -> RepoResult<Vec<SalesReturnItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, SalesReturnItem>(
            &format!("SELECT {ITEM_COLUMNS} FROM sales_return_items WHERE return_id = $1 ORDER BY id"),
        )
        .bind(return_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }
}
