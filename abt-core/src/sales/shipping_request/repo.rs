use chrono::NaiveDate;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const SR_COLUMNS: &str = "id, doc_number, order_id, customer_id, request_date, expected_ship_date, status, shipping_address, carrier, tracking_number, remark, operator_id, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, shipped_qty, description";

// ---------------------------------------------------------------------------
// ShippingRequestRepo
// ---------------------------------------------------------------------------

pub struct ShippingRequestRepo;

impl ShippingRequestRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        doc_number: &str,
        order_id: i64,
        customer_id: i64,
        expected_ship_date: Option<NaiveDate>,
        shipping_address: &str,
        remark: &str,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO shipping_requests (doc_number, order_id, customer_id, expected_ship_date, shipping_address, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(order_id)
        .bind(customer_id)
        .bind(expected_ship_date)
        .bind(shipping_address)
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
    ) -> Result<Option<ShippingRequest>> {
        let sr = sqlx::query_as::<sqlx::Postgres, ShippingRequest>(
            &format!("SELECT {SR_COLUMNS} FROM shipping_requests WHERE id = $1 AND deleted_at IS NULL"),
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
        status: ShippingStatus,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE shipping_requests SET status = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(status.as_i16())
        .execute(executor)
        .await?;
        Ok(())
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateShippingReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.expected_ship_date.is_some() {
            sets.push(format!("expected_ship_date = ${param_idx}"));
            param_idx += 1;
        }
        if req.shipping_address.is_some() {
            sets.push(format!("shipping_address = ${param_idx}"));
            param_idx += 1;
        }
        if req.carrier.is_some() {
            sets.push(format!("carrier = ${param_idx}"));
            param_idx += 1;
        }
        if req.tracking_number.is_some() {
            sets.push(format!("tracking_number = ${param_idx}"));
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
            "UPDATE shipping_requests SET {} WHERE id = $1 AND deleted_at IS NULL",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql).bind(id);

        if let Some(v) = req.expected_ship_date {
            q = q.bind(v);
        }
        if let Some(ref v) = req.shipping_address {
            q = q.bind(v);
        }
        if let Some(ref v) = req.carrier {
            q = q.bind(v);
        }
        if let Some(ref v) = req.tracking_number {
            q = q.bind(v);
        }
        if let Some(ref v) = req.remark {
            q = q.bind(v);
        }

        q.execute(executor).await?;
        Ok(())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &ShippingQuery,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<PaginatedResult<ShippingRequest>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let order_param = if let Some(oid) = filter.order_id {
            param_idx += 1;
            conditions.push(format!("order_id = ${param_idx}"));
            Some(oid)
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

        let customer_param = if let Some(cid) = filter.customer_id {
            param_idx += 1;
            conditions.push(format!("customer_id = ${param_idx}"));
            Some(cid)
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

        let count_sql = format!("SELECT COUNT(*) FROM shipping_requests WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(v) = order_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        if let Some(v) = customer_param { count_q = count_q.bind(v); }
        if let Some(v) = scope_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {SR_COLUMNS} FROM shipping_requests WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, ShippingRequest>(&data_sql);
        if let Some(v) = order_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(ref v) = keyword_param { data_q = data_q.bind(v); }
        if let Some(v) = customer_param { data_q = data_q.bind(v); }
        if let Some(v) = scope_param { data_q = data_q.bind(v); }
        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}

// ---------------------------------------------------------------------------
// ShippingRequestItemRepo
// ---------------------------------------------------------------------------

pub struct ShippingRequestItemRepo;

impl ShippingRequestItemRepo {
    pub async fn create_batch(
        &self,
        executor: PgExecutor<'_>,
        shipping_request_id: i64,
        items: &[ShippingItemInput],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO shipping_request_items (shipping_request_id, line_no, order_item_id, product_id, warehouse_id, requested_qty, description)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(shipping_request_id)
            .bind(item.line_no)
            .bind(item.order_item_id)
            .bind(item.product_id)
            .bind(item.warehouse_id)
            .bind(item.requested_qty)
            .bind(&item.description)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn find_by_shipping_request_id(
        &self,
        executor: PgExecutor<'_>,
        shipping_request_id: i64,
    ) -> Result<Vec<ShippingRequestItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, ShippingRequestItem>(
            &format!("SELECT {ITEM_COLUMNS} FROM shipping_request_items WHERE shipping_request_id = $1 ORDER BY line_no"),
        )
        .bind(shipping_request_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }

    pub async fn update_shipped_qty(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        shipped_qty: rust_decimal::Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE shipping_request_items SET shipped_qty = $2 WHERE id = $1",
        )
        .bind(id)
        .bind(shipped_qty)
        .execute(executor)
        .await?;
        Ok(())
    }
}
