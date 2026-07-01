use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::{DomainError, Result};
use sqlx::Row;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const ORDER_COLUMNS: &str = "id, doc_number, customer_id, contact_id, sales_rep_id, order_date, status, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id, created_at, updated_at, deleted_at";

const ITEM_COLUMNS: &str = "id, order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, shipped_qty, cancelled_qty, returned_qty, line_status, version, delivery_date";

// ---------------------------------------------------------------------------
// SalesOrderRepo
// ---------------------------------------------------------------------------

pub struct SalesOrderRepo;

impl SalesOrderRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        params: &CreateSalesOrderParams<'_>,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO sales_orders (doc_number, customer_id, contact_id, sales_rep_id, total_amount, total_cost, payment_terms, delivery_terms, delivery_address, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               RETURNING id"#,
        )
        .bind(params.doc_number)
        .bind(params.customer_id)
        .bind(params.contact_id)
        .bind(params.sales_rep_id)
        .bind(params.total_amount)
        .bind(params.total_cost)
        .bind(params.payment_terms)
        .bind(params.delivery_terms)
        .bind(params.delivery_address)
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

    /// 按 ID 查询订单号（供跨模块 Event Handler 使用）
    pub async fn find_doc_number_by_id(
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT doc_number FROM sales_orders WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(row.map(|r| r.try_get::<String, _>("doc_number")).transpose()?)
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
                r#"INSERT INTO sales_order_items (order_id, line_no, product_id, description, quantity, unit, unit_price, unit_cost, discount_rate, amount, cancelled_qty, line_status, version, delivery_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, 1, 1, $11)"#,
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

    /// 按多个订单 ID 批量取明细（用于列表/搜索场景，避免 N+1）。
    pub async fn find_by_order_ids(
        &self,
        executor: PgExecutor<'_>,
        order_ids: &[i64],
    ) -> Result<Vec<SalesOrderItem>> {
        if order_ids.is_empty() {
            return Ok(Vec::new());
        }
        let items = sqlx::query_as::<sqlx::Postgres, SalesOrderItem>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {ITEM_COLUMNS} FROM sales_order_items WHERE order_id = ANY($1) ORDER BY order_id, line_no"
            )),
        )
        .bind(order_ids)
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

    /// 批量更新行状态（乐观锁）
    pub async fn batch_update_line_status(
        &self,
        executor: PgExecutor<'_>,
        updates: &[(i64, SalesOrderLineStatus, i32)],  // (id, new_status, expected_version)
    ) -> Result<()> {
        for (id, status, expected_version) in updates {
            let rows = sqlx::query(
                r#"UPDATE sales_order_items
                   SET line_status = $1, version = version + 1
                   WHERE id = $2 AND version = $3"#,
            )
            .bind(status.as_i16())
            .bind(id)
            .bind(expected_version)
            .execute(&mut *executor)
            .await?;

            if rows.rows_affected() == 0 {
                return Err(DomainError::ConcurrentConflict);
            }
        }
        Ok(())
    }

    /// 取消订单行（增加 cancelled_qty，乐观锁）
    pub async fn cancel_line(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        add_cancelled_qty: Decimal,
        new_line_status: SalesOrderLineStatus,
        expected_version: i32,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"UPDATE sales_order_items
               SET cancelled_qty = cancelled_qty + $1,
                   line_status = $2,
                   version = version + 1
               WHERE id = $3 AND version = $4
                 AND quantity - shipped_qty - cancelled_qty - $1 >= 0"#,
        )
        .bind(add_cancelled_qty)
        .bind(new_line_status.as_i16())
        .bind(id)
        .bind(expected_version)
        .execute(executor)
        .await?;

        if rows.rows_affected() == 0 {
            return Err(DomainError::ConcurrentConflict);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FulfillmentPlanLineRepo
// ---------------------------------------------------------------------------

pub struct FulfillmentPlanLineRepo;

const FP_COLUMNS: &str = "id, order_id, order_line_id, product_id, acquire_channel, required_qty, reserved_qty, shortage_qty, status, source_doc_type, source_doc_id, reservation_details, required_date, version, created_at, updated_at";

impl FulfillmentPlanLineRepo {
    /// 批量插入履行计划行
    pub async fn create_batch(
        executor: PgExecutor<'_>,
        lines: &[FulfillmentPlanLineInput],
    ) -> Result<Vec<i64>> {
        let mut ids = Vec::with_capacity(lines.len());
        for line in lines {
            let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
                r#"INSERT INTO fulfillment_plan_lines
                   (order_id, order_line_id, product_id, acquire_channel, required_qty, reserved_qty, shortage_qty, status, required_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                   RETURNING id"#,
            )
            .bind(line.order_id)
            .bind(line.order_line_id)
            .bind(line.product_id)
            .bind(line.acquire_channel.as_i16())
            .bind(line.required_qty)
            .bind(line.reserved_qty)
            .bind(line.shortage_qty)
            .bind(line.status.as_i16())
            .bind(line.required_date)
            .fetch_one(&mut *executor)
            .await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// 按订单ID查询履行计划行
    pub async fn find_by_order_id(
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<FulfillmentPlanLine>> {
        let lines = sqlx::query_as::<sqlx::Postgres, FulfillmentPlanLine>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {FP_COLUMNS} FROM fulfillment_plan_lines WHERE order_id = $1"
            )),
        )
        .bind(order_id)
        .fetch_all(executor)
        .await?;
        Ok(lines)
    }

    /// 按订单行ID查询（唯一）
    pub async fn find_by_order_line_id(
        executor: PgExecutor<'_>,
        order_line_id: i64,
    ) -> Result<Option<FulfillmentPlanLine>> {
        let line = sqlx::query_as::<sqlx::Postgres, FulfillmentPlanLine>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {FP_COLUMNS} FROM fulfillment_plan_lines WHERE order_line_id = $1"
            )),
        )
        .bind(order_line_id)
        .fetch_optional(executor)
        .await?;
        Ok(line)
    }

    /// 批量按订单行ID查询（避免逐个 find_by_order_line_id 的 N+1）。结果含 order_line_id，调用方按需建 map。
    pub async fn find_by_order_line_ids(
        executor: PgExecutor<'_>,
        order_line_ids: &[i64],
    ) -> Result<Vec<FulfillmentPlanLine>> {
        if order_line_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<sqlx::Postgres, FulfillmentPlanLine>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {FP_COLUMNS} FROM fulfillment_plan_lines WHERE order_line_id = ANY($1)"
            )),
        )
        .bind(order_line_ids)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 更新状态（乐观锁）
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: FulfillmentLineStatus,
        expected_version: i32,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"UPDATE fulfillment_plan_lines
               SET status = $1, version = version + 1, updated_at = NOW()
               WHERE id = $2 AND version = $3"#,
        )
        .bind(status.as_i16())
        .bind(id)
        .bind(expected_version)
        .execute(executor)
        .await?;

        if rows.rows_affected() == 0 {
            return Err(DomainError::ConcurrentConflict);
        }
        Ok(())
    }

    /// 更新下游单据关联
    pub async fn update_source_doc(
        executor: PgExecutor<'_>,
        id: i64,
        source_doc_type: i16,
        source_doc_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE fulfillment_plan_lines
               SET source_doc_type = $1, source_doc_id = $2, updated_at = NOW()
               WHERE id = $3"#,
        )
        .bind(source_doc_type)
        .bind(source_doc_id)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DemandRepo
// ---------------------------------------------------------------------------

pub struct DemandRepo;

const DEMAND_COLUMNS: &str = "id, demand_type, source_type, source_id, source_line_id, product_id, acquire_channel, required_qty, required_date, status, target_doc_type, target_doc_id, priority, cascade_from_product_id, remark, operator_id, created_at, updated_at, deleted_at";

impl DemandRepo {
    /// 创建需求
    pub async fn create(
        executor: PgExecutor<'_>,
        input: &DemandInput,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO demands
               (demand_type, source_type, source_id, source_line_id, product_id,
                acquire_channel, required_qty, required_date, status, priority,
                cascade_from_product_id, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1, $9, $10, $11, $12)
               RETURNING id"#,
        )
        .bind(input.demand_type)
        .bind(input.source_type)
        .bind(input.source_id)
        .bind(input.source_line_id)
        .bind(input.product_id)
        .bind(input.acquire_channel)
        .bind(input.required_qty)
        .bind(input.required_date)
        .bind(input.priority)
        .bind(input.cascade_from_product_id)
        .bind(&input.remark)
        .bind(input.operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 按 ID 查询
    pub async fn find_by_id(
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<Demand>> {
        let demand = sqlx::query_as::<sqlx::Postgres, Demand>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {DEMAND_COLUMNS} FROM demands WHERE id = $1 AND deleted_at IS NULL"
            )),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(demand)
    }

    /// 按来源行查询
    pub async fn find_by_source_line(
        executor: PgExecutor<'_>,
        source_type: i16,
        source_line_id: i64,
    ) -> Result<Vec<Demand>> {
        let demands = sqlx::query_as::<sqlx::Postgres, Demand>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {DEMAND_COLUMNS} FROM demands WHERE source_type = $1 AND source_line_id = $2 AND deleted_at IS NULL"
            )),
        )
        .bind(source_type)
        .bind(source_line_id)
        .fetch_all(executor)
        .await?;
        Ok(demands)
    }

    /// 按来源单据查询所有需求（如某销售订单的所有 demand）
    pub async fn find_by_source(
        executor: PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
    ) -> Result<Vec<Demand>> {
        let demands = sqlx::query_as::<sqlx::Postgres, Demand>(
            sqlx::AssertSqlSafe(format!(
                "SELECT {DEMAND_COLUMNS} FROM demands WHERE source_type = $1 AND source_id = $2 AND deleted_at IS NULL"
            )),
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_all(executor)
        .await?;
        Ok(demands)
    }

    /// 更新状态
    pub async fn update_status(
        executor: PgExecutor<'_>,
        id: i64,
        status: DemandStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE demands SET status = $1, updated_at = NOW() WHERE id = $2"#,
        )
        .bind(status.as_i16())
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 更新下游单据关联
    pub async fn update_target_doc(
        executor: PgExecutor<'_>,
        id: i64,
        target_doc_type: i16,
        target_doc_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE demands
               SET target_doc_type = $1, target_doc_id = $2, updated_at = NOW()
               WHERE id = $3"#,
        )
        .bind(target_doc_type)
        .bind(target_doc_id)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 批量释放下游单据关联的所有需求回池（status→Pending，清 target_doc）。
    /// 用于工单/采购单取消时回退需求 —— 与 update_target_doc 对称。
    /// 返回被释放需求的 (id, source_line_id, product_id, acquire_channel)，供调用方发事件。
    pub async fn release_by_target_doc(
        executor: PgExecutor<'_>,
        target_doc_type: i16,
        target_doc_id: i64,
    ) -> Result<Vec<(i64, Option<i64>, i64, Option<i16>)>> {
        let rows = sqlx::query_as::<_, (i64, Option<i64>, i64, Option<i16>)>(
            r#"UPDATE demands
               SET status = 1, target_doc_type = NULL, target_doc_id = NULL, updated_at = NOW()
               WHERE target_doc_id = $1 AND target_doc_type = $2 AND deleted_at IS NULL
               RETURNING id, source_line_id, product_id, acquire_channel"#,
        )
        .bind(target_doc_id)
        .bind(target_doc_type)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 对账查询：查找履行计划行状态与 demand 状态不一致的记录
    pub async fn find_mismatched(
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<(i64, i64)>> {
        let rows = sqlx::query_as::<sqlx::Postgres, (i64, i64)>(
            r#"SELECT fp.id, d.id
               FROM fulfillment_plan_lines fp
               JOIN demands d ON d.source_type = 2
                 AND d.source_line_id = fp.order_line_id
                 AND d.deleted_at IS NULL
               WHERE fp.order_id = $1
                 AND fp.status IN (3, 4)
                 AND d.status IN (1, 5)"#,
        )
        .bind(order_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 检查是否已存在同来源的 BOM 级联需求
    /// 参考 Odoo `_make_mo_get_domain`：查已有同源需求避免重复创建
    pub async fn find_cascade_existing(
        executor: PgExecutor<'_>,
        source_id: i64,
        source_line_id: i64,
        product_id: i64,
        cascade_from_product_id: i64,
    ) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM demands
               WHERE source_id = $1
                 AND source_line_id = $2
                 AND product_id = $3
                 AND cascade_from_product_id = $4
                 AND demand_type = 2
                 AND deleted_at IS NULL"#,
        )
        .bind(source_id)
        .bind(source_line_id)
        .bind(product_id)
        .bind(cascade_from_product_id)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }

    /// 批量查询已存在的 BOM 级联需求（消除 N+1）
    /// 返回已存在的 (product_id, cascade_from_product_id) 集合
    pub async fn find_cascade_existing_batch(
        executor: PgExecutor<'_>,
        source_id: i64,
        source_line_id: i64,
        cascade_from_product_id: i64,
    ) -> Result<std::collections::HashSet<i64>> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"SELECT product_id FROM demands
               WHERE source_id = $1
                 AND source_line_id = $2
                 AND cascade_from_product_id = $3
                 AND demand_type = 2
                 AND deleted_at IS NULL"#,
        )
        .bind(source_id)
        .bind(source_line_id)
        .bind(cascade_from_product_id)
        .fetch_all(executor)
        .await?;
        Ok(rows.into_iter().map(|(pid,)| pid).collect())
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
