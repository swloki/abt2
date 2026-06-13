use sqlx::FromRow;
use crate::shared::types::Result;

use super::super::enums::WorkOrderStatus;
use super::model::{WorkOrder, WorkOrderFilter};
use crate::shared::types::pagination::PaginatedResult;

pub struct WorkOrderRepo;

impl WorkOrderRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        req: &super::model::CreateWorkOrderReq,
        status: WorkOrderStatus,
        operator_id: i64,
    ) -> Result<WorkOrder> {
        let remark = req.remark.as_deref().unwrap_or("");
        let row = sqlx::query(
            r#"
            INSERT INTO work_orders
                (doc_number, plan_item_id, product_id, bom_snapshot_id, routing_id,
                 planned_qty, scheduled_start, scheduled_end, status, work_center_id,
                 sales_order_id, version, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 1, $12, $13)
            RETURNING id, doc_number, plan_item_id, product_id, bom_snapshot_id, routing_id,
                      planned_qty, scheduled_start, scheduled_end, status, work_center_id,
                      sales_order_id, version, remark, operator_id, created_at, updated_at, deleted_at,
                      NULL::int AS completed_steps, NULL::int AS total_steps,
                      NULL::bigint AS source_plan_id, NULL::text AS source_plan_doc,
                      NULL::text AS source_so_doc, NULL::text AS source_customer
            "#,
        )
        .bind(doc_number)
        .bind(req.plan_item_id)
        .bind(req.product_id)
        .bind(req.bom_snapshot_id)
        .bind(req.routing_id)
        .bind(req.planned_qty)
        .bind(req.scheduled_start)
        .bind(req.scheduled_end)
        .bind(status)
        .bind(req.work_center_id)
        .bind(req.sales_order_id)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(WorkOrder::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<WorkOrder>> {
        let row = sqlx::query(
            r#"
            SELECT wo.id, wo.doc_number, wo.plan_item_id, wo.product_id, wo.bom_snapshot_id, wo.routing_id,
                   wo.planned_qty, wo.scheduled_start, wo.scheduled_end, wo.status, wo.work_center_id,
                   wo.sales_order_id, wo.version, wo.remark, wo.operator_id, wo.created_at, wo.updated_at, wo.deleted_at,
                   (SELECT COUNT(*)::int FROM work_order_routings r WHERE r.work_order_id = wo.id) AS total_steps,
                   (SELECT COUNT(*)::int FROM work_order_routings r WHERE r.work_order_id = wo.id AND r.status = 3) AS completed_steps,
                   pp.id AS source_plan_id, pp.doc_number AS source_plan_doc,
                   so.doc_number AS source_so_doc, c.customer_name AS source_customer
            FROM work_orders wo
            LEFT JOIN production_plan_items ppi ON ppi.id = wo.plan_item_id
            LEFT JOIN production_plans pp ON pp.id = ppi.plan_id
            LEFT JOIN sales_orders so ON so.id = wo.sales_order_id
            LEFT JOIN customers c ON c.customer_id = so.customer_id
            WHERE wo.id = $1 AND wo.deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| WorkOrder::from_row(&r).map_err(Into::into)).transpose()

    }

    /// 乐观锁更新状态。返回 true 表示更新成功，false 表示版本不匹配或行不存在。
    pub async fn update_status_with_version(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: WorkOrderStatus,
        expected_version: i32,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE work_orders
            SET status = $2, version = version + 1, updated_at = NOW()
            WHERE id = $1 AND version = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(expected_version)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &WorkOrderFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WorkOrder>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["wo.deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.status = ${param_idx}"));
        }
        if filter.product_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.product_id = ${param_idx}"));
        }
        if filter.keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.doc_number ILIKE ${param_idx}"));
        }
        if filter.date_from.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.scheduled_start >= ${param_idx}"));
        }
        if filter.date_to.is_some() {
            param_idx += 1;
            where_clauses.push(format!("wo.scheduled_end <= ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM work_orders wo WHERE {where_sql}");
        let data_sql = format!(
            "SELECT wo.id, wo.doc_number, wo.plan_item_id, wo.product_id, wo.bom_snapshot_id, wo.routing_id, \
             wo.planned_qty, wo.scheduled_start, wo.scheduled_end, wo.status, wo.work_center_id, \
             wo.sales_order_id, wo.version, wo.remark, wo.operator_id, wo.created_at, wo.updated_at, wo.deleted_at, \
             (SELECT COUNT(*)::int FROM work_order_routings r WHERE r.work_order_id = wo.id) AS total_steps, \
             (SELECT COUNT(*)::int FROM work_order_routings r WHERE r.work_order_id = wo.id AND r.status = 3) AS completed_steps, \
             pp.id AS source_plan_id, pp.doc_number AS source_plan_doc, \
             so.doc_number AS source_so_doc, c.customer_name AS source_customer \
             FROM work_orders wo \
             LEFT JOIN production_plan_items ppi ON ppi.id = wo.plan_item_id \
             LEFT JOIN production_plans pp ON pp.id = ppi.plan_id \
             LEFT JOIN sales_orders so ON so.id = wo.sales_order_id \
             LEFT JOIN customers c ON c.customer_id = so.customer_id \
             WHERE {where_sql} \
             ORDER BY wo.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.product_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(ref v) = filter.keyword {
            let pattern = format!("%{v}%");
            count_q = count_q.bind(pattern.clone());
            data_q = data_q.bind(pattern);
        }
        if let Some(v) = filter.date_from {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.date_to {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<WorkOrder> = rows
            .iter()
            .filter_map(|r| WorkOrder::from_row(r).ok())
            .collect();

        let total_pages = if page_size == 0 {
            0
        } else {
            (total as u64).div_ceil(page_size as u64) as u32
        };

        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }

    /// 软删除工单（标记 deleted_at + 状态改为 Cancelled）
    pub async fn soft_delete(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE work_orders
            SET deleted_at = NOW(), status = $2, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(WorkOrderStatus::Cancelled)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 软删除工单下的所有生产批次（标记为 Cancelled）
    pub async fn soft_delete_batches(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE production_batches
            SET status = 6, updated_at = NOW()
            WHERE work_order_id = $1 AND status != 6
            "#,
        )
        .bind(work_order_id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 更新工单的 BOM 快照 ID
    pub async fn update_bom_snapshot_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        bom_snapshot_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE work_orders SET bom_snapshot_id = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(bom_snapshot_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 更新工单的工艺路线 ID
    pub async fn update_routing_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        routing_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE work_orders SET routing_id = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(routing_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
    /// 按生产计划 ID 查询关联工单（通过 plan_item_id JOIN production_plan_items）
    pub async fn list_by_plan(
        executor: &mut sqlx::postgres::PgConnection,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>> {
        let rows = sqlx::query(
            r#"
            SELECT wo.id, wo.doc_number, wo.plan_item_id, wo.product_id, wo.bom_snapshot_id, wo.routing_id,
                   wo.planned_qty, wo.scheduled_start, wo.scheduled_end, wo.status, wo.work_center_id,
                   wo.sales_order_id, wo.version, wo.remark, wo.operator_id, wo.created_at, wo.updated_at, wo.deleted_at,
                   (SELECT COUNT(*)::int FROM work_order_routings r WHERE r.work_order_id = wo.id) AS total_steps,
                   (SELECT COUNT(*)::int FROM work_order_routings r WHERE r.work_order_id = wo.id AND r.status = 3) AS completed_steps,
                   pp.id AS source_plan_id, pp.doc_number AS source_plan_doc,
                   so.doc_number AS source_so_doc, c.customer_name AS source_customer
            FROM work_orders wo
            LEFT JOIN production_plan_items ppi ON ppi.id = wo.plan_item_id
            LEFT JOIN production_plans pp ON pp.id = ppi.plan_id
            LEFT JOIN sales_orders so ON so.id = wo.sales_order_id
            LEFT JOIN customers c ON c.customer_id = so.customer_id
            WHERE wo.plan_item_id IN (SELECT id FROM production_plan_items WHERE plan_id = $1)
              AND wo.deleted_at IS NULL
            ORDER BY wo.id
            "#,
        )
        .bind(plan_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .map(|r| WorkOrder::from_row(r).map_err(Into::into))
            .collect()
    }

    /// 条件状态更新（无乐观锁，用于事务内传播）。
    /// 仅当当前状态匹配 from 时才更新为 to，返回是否实际更新。
    pub async fn update_status_conditional(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        from: WorkOrderStatus,
        to: WorkOrderStatus,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE work_orders
            SET status = $3, version = version + 1, updated_at = NOW()
            WHERE id = $1 AND status = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(from)
        .bind(to)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
