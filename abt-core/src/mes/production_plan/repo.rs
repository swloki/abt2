use sqlx::FromRow;

use super::super::enums::{PlanItemStatus, PlanStatus};
use super::model::*;
use crate::shared::types::pagination::PaginatedResult;

pub struct ProductionPlanRepo;

impl ProductionPlanRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreatePlanReq,
        doc_number: &str,
        operator_id: i64,
    ) -> Result<ProductionPlan, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO production_plans
                (doc_number, plan_type, status, plan_date, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, doc_number, plan_date, plan_type, status, remark,
                      operator_id, created_at, updated_at, deleted_at
            "#,
        )
        .bind(doc_number)
        .bind(req.plan_type)
        .bind(PlanStatus::Draft)
        .bind(req.plan_date)
        .bind(req.remark.as_deref().unwrap_or(""))
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        ProductionPlan::from_row(&row)
    }

    pub async fn insert_items(
        executor: &mut sqlx::postgres::PgConnection,
        plan_id: i64,
        items: &[CreatePlanItemReq],
    ) -> Result<Vec<ProductionPlanItem>, sqlx::Error> {
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let row = sqlx::query(
                r#"
                INSERT INTO production_plan_items
                    (plan_id, product_id, planned_qty, scheduled_start, scheduled_end,
                     sales_order_id, sales_order_item_id, bom_snapshot_id, routing_id,
                     work_center_id, priority, status)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                RETURNING id, plan_id, product_id, planned_qty, scheduled_start, scheduled_end,
                          sales_order_id, sales_order_item_id, bom_snapshot_id, routing_id,
                          work_center_id, priority, status
                "#,
            )
            .bind(plan_id)
            .bind(item.product_id)
            .bind(item.planned_qty)
            .bind(item.scheduled_start)
            .bind(item.scheduled_end)
            .bind(item.sales_order_id)
            .bind(item.sales_order_item_id)
            .bind(item.bom_snapshot_id)
            .bind(item.routing_id)
            .bind(item.work_center_id)
            .bind(item.priority)
            .bind(PlanItemStatus::Planned)
            .fetch_one(&mut *executor)
            .await?;

            results.push(ProductionPlanItem::from_row(&row)?);
        }
        Ok(results)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<ProductionPlan>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, plan_date, plan_type, status, remark,
                   operator_id, created_at, updated_at, deleted_at
            FROM production_plans
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ProductionPlan::from_row(&r)).transpose()
    }

    pub async fn get_items_by_plan_id(
        executor: &mut sqlx::postgres::PgConnection,
        plan_id: i64,
    ) -> Result<Vec<ProductionPlanItem>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, plan_id, product_id, planned_qty, scheduled_start, scheduled_end,
                   sales_order_id, sales_order_item_id, bom_snapshot_id, routing_id,
                   work_center_id, priority, status
            FROM production_plan_items
            WHERE plan_id = $1
            ORDER BY id
            "#,
        )
        .bind(plan_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| ProductionPlanItem::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: PlanStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE production_plans
            SET status = $2, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(())
    }

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &PlanFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ProductionPlan>, sqlx::Error> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 1u32;

        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.plan_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("plan_type = ${param_idx}"));
        }
        if filter.keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("doc_number ILIKE ${param_idx}"));
        }
        if filter.date_from.is_some() {
            param_idx += 1;
            where_clauses.push(format!("plan_date >= ${param_idx}"));
        }
        if filter.date_to.is_some() {
            param_idx += 1;
            where_clauses.push(format!("plan_date <= ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) AS total FROM production_plans WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, plan_date, plan_type, status, remark, \
             operator_id, created_at, updated_at, deleted_at \
             FROM production_plans WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
        let mut data_q = sqlx::query(&data_sql);

        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.plan_type {
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
        let items: Vec<ProductionPlan> = rows
            .iter()
            .filter_map(|r| ProductionPlan::from_row(r).ok())
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
}
