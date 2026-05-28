use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const ORDER_COLUMNS: &str = "id, doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, returned_qty, delivery_date";

// ---------------------------------------------------------------------------
// SalesOrderRepo
// ---------------------------------------------------------------------------

pub struct SalesOrderRepo;

impl SalesOrderRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        doc_number: &str,
        customer_id: i64,
        contact_id: i64,
        sales_rep_id: i64,
        total_amount: Decimal,
        total_cost: Decimal,
        payment_terms: &str,
        delivery_terms: &str,
        delivery_address: &str,
        remark: &str,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(customer_id)
        .bind(contact_id)
        .bind(sales_rep_id)
        .bind(total_amount)
        .bind(total_cost)
        .bind(payment_terms)
        .bind(delivery_terms)
        .bind(delivery_address)
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
    ) -> Result<Option<SalesOrder>> {
        let order = sqlx::query_as::<sqlx::Postgres, SalesOrder>(
            sqlx::AssertSqlSafe(format!("SELECT {ORDER_COLUMNS} FROM sales_orders WHERE id = $1 AND deleted_at IS NULL")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(order)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateSalesOrderReq,
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
        if req.payment_terms.is_some() {
            sets.push(format!("payment_terms = ${param_idx}"));
            param_idx += 1;
        }
        if req.delivery_terms.is_some() {
            sets.push(format!("delivery_terms = ${param_idx}"));
            param_idx += 1;
        }
        if req.delivery_address.is_some() {
            sets.push(format!("delivery_address = ${param_idx}"));
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
            "UPDATE sales_orders SET {} WHERE id = $1 AND deleted_at IS NULL",
            sets.join(", ")
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(v) = req.customer_id {
            q = q.bind(v);
        }
        if let Some(v) = req.contact_id {
            q = q.bind(v);
        }
        if let Some(ref v) = req.payment_terms {
            q = q.bind(v);
        }
        if let Some(ref v) = req.delivery_terms {
            q = q.bind(v);
        }
        if let Some(ref v) = req.delivery_address {
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
        status: SalesOrderStatus,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE sales_orders SET status = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
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
        total_amount: Decimal,
        total_cost: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE sales_orders SET total_amount = $2, total_cost = $3, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(total_amount)
        .bind(total_cost)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn soft_delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE sales_orders SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &SalesOrderQuery,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        _scope_department_id: Option<i64>,
    ) -> Result<PaginatedResult<SalesOrder>> {
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
            conditions.push(format!("order_date >= ${param_idx}"));
            Some(date_from)
        } else {
            None
        };

        let date_to_param = if let Some(date_to) = filter.date_to {
            param_idx += 1;
            conditions.push(format!("order_date <= ${param_idx}"));
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

        let count_sql = format!("SELECT COUNT(*) FROM sales_orders WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = customer_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(v) = date_from_param { count_q = count_q.bind(v); }
        if let Some(v) = date_to_param { count_q = count_q.bind(v); }
        if let Some(ref v) = keyword_param { count_q = count_q.bind(v); }
        if let Some(v) = scope_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {ORDER_COLUMNS} FROM sales_orders WHERE {where_clause} ORDER BY id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, SalesOrder>(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = customer_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(v) = date_from_param { data_q = data_q.bind(v); }
        if let Some(v) = date_to_param { data_q = data_q.bind(v); }
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
// SalesOrderItemRepo
// ---------------------------------------------------------------------------

pub struct SalesOrderItemRepo;

impl SalesOrderItemRepo {
    pub async fn create_batch(
        &self,
        executor: PgExecutor<'_>,
        order_id: i64,
        items: &[SalesOrderItemInput],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, delivery_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
            )
            .bind(order_id)
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

    pub async fn find_by_order_id(
        &self,
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<SalesOrderItem>> {
        let items = sqlx::query_as::<sqlx::Postgres, SalesOrderItem>(
            sqlx::AssertSqlSafe(format!("SELECT {ITEM_COLUMNS} FROM sales_order_items WHERE order_id = $1 ORDER BY line_no")),
        )
        .bind(order_id)
        .fetch_all(executor)
        .await?;
        Ok(items)
    }

    pub async fn delete_by_order_id(
        &self,
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM sales_order_items WHERE order_id = $1")
            .bind(order_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn update_shipped_qty(
        &self,
        executor: PgExecutor<'_>,
        item_id: i64,
        shipped_qty: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE sales_order_items SET shipped_qty = shipped_qty + $2 WHERE id = $1",
        )
        .bind(item_id)
        .bind(shipped_qty)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn update_returned_qty(
        &self,
        executor: PgExecutor<'_>,
        item_id: i64,
        returned_qty: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE sales_order_items SET returned_qty = returned_qty + $2 WHERE id = $1",
        )
        .bind(item_id)
        .bind(returned_qty)
        .execute(executor)
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SAVEPOINT helpers
// ---------------------------------------------------------------------------

pub async fn savepoint(db: PgExecutor<'_>, name: &str) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!("SAVEPOINT {name}")))
        .execute(&mut *db)
        .await?;
    Ok(())
}

pub async fn release_savepoint(db: PgExecutor<'_>, name: &str) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!("RELEASE SAVEPOINT {name}")))
        .execute(&mut *db)
        .await?;
    Ok(())
}

pub async fn rollback_savepoint(db: PgExecutor<'_>, name: &str) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!("ROLLBACK TO SAVEPOINT {name}")))
        .execute(&mut *db)
        .await?;
    Ok(())
}
