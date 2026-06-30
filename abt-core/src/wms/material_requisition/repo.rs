use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::shared::types::Result;

use super::super::enums::RequisitionStatus;
use super::model::{MaterialReqItem, MaterialRequisition, RequisitionFilter};
use crate::shared::types::pagination::PaginatedResult;

pub struct MaterialRequisitionRepo;

impl MaterialRequisitionRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        doc_number: &str,
        work_order_id: i64,
        requisition_date: NaiveDate,
        warehouse_id: i64,
        operator_id: i64,
    ) -> Result<MaterialRequisition> {
        let row = sqlx::query(
            r#"
            INSERT INTO material_requisitions
                (doc_number, work_order_id, requisition_date, status, warehouse_id, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, doc_number, work_order_id, requisition_date, status,
                      warehouse_id, operator_id, created_at, updated_at, deleted_at
            "#,
        )
        .bind(doc_number)
        .bind(work_order_id)
        .bind(requisition_date)
        .bind(RequisitionStatus::Draft)
        .bind(warehouse_id)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(MaterialRequisition::from_row(&row)?)
    }

    pub async fn insert_item(
        executor: &mut sqlx::postgres::PgConnection,
        requisition_id: i64,
        product_id: i64,
        requested_qty: Decimal,
        operation_id: Option<i64>,
        batch_id: Option<i64>,
    ) -> Result<MaterialReqItem> {
        let row = sqlx::query(
            r#"
            INSERT INTO material_requisition_items
                (requisition_id, product_id, requested_qty, issued_qty, variance_qty, operation_id, batch_id)
            VALUES ($1, $2, $3, 0, 0, $4, $5)
            RETURNING id, requisition_id, product_id, requested_qty, issued_qty, variance_qty, bin_id, operation_id, batch_id
            "#,
        )
        .bind(requisition_id)
        .bind(product_id)
        .bind(requested_qty)
        .bind(operation_id)
        .bind(batch_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(MaterialReqItem::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<MaterialRequisition>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, requisition_date, status,
                   warehouse_id, operator_id, created_at, updated_at, deleted_at
            FROM material_requisitions
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| MaterialRequisition::from_row(&r).map_err(Into::into)).transpose()

    }

    pub async fn get_items(
        executor: &mut sqlx::postgres::PgConnection,
        requisition_id: i64,
    ) -> Result<Vec<MaterialReqItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, requisition_id, product_id, requested_qty, issued_qty, variance_qty, bin_id, operation_id, batch_id
            FROM material_requisition_items
            WHERE requisition_id = $1
            ORDER BY id
            "#,
        )
        .bind(requisition_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .filter_map(|r| MaterialReqItem::from_row(r).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(Ok)
            .collect()
    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: RequisitionStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE material_requisitions
            SET status = $2, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn update_item_issued(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        issued_qty: Decimal,
        variance_qty: Decimal,
        bin_id: Option<i64>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE material_requisition_items
            SET issued_qty = $2, variance_qty = $3, bin_id = $4
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .bind(issued_qty)
        .bind(variance_qty)
        .bind(bin_id)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &RequisitionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<MaterialRequisition>> {
        let offset = (page.saturating_sub(1)) * page_size;

        let mut where_clauses = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        if filter.doc_number.is_some() {
            param_idx += 1;
            where_clauses.push(format!("doc_number ILIKE ${param_idx}"));
        }
        if filter.status.is_some() {
            param_idx += 1;
            where_clauses.push(format!("status = ${param_idx}"));
        }
        if filter.work_order_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("work_order_id = ${param_idx}"));
        }
        if filter.warehouse_id.is_some() {
            param_idx += 1;
            where_clauses.push(format!("warehouse_id = ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!("SELECT COUNT(*) as total FROM material_requisitions WHERE {where_sql}");
        let data_sql = format!(
            "SELECT id, doc_number, work_order_id, requisition_date, status, \
             warehouse_id, operator_id, created_at, updated_at, deleted_at \
             FROM material_requisitions WHERE {where_sql} \
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));

        if let Some(ref v) = filter.doc_number {
            let pattern = format!("%{v}%");
            count_q = count_q.bind(pattern.clone());
            data_q = data_q.bind(pattern);
        }
        if let Some(v) = filter.status {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.work_order_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        if let Some(v) = filter.warehouse_id {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<MaterialRequisition> = rows
            .iter()
            .filter_map(|r| MaterialRequisition::from_row(r).ok())
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

    pub async fn soft_delete(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE material_requisitions
            SET deleted_at = NOW(), status = $2, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(RequisitionStatus::Cancelled)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询批次已领料的工序 routing_id 集合（判断工序是否已领料，驱动批次矩阵动作位）。
    /// 排除已取消的领料单。
    pub async fn find_routing_ids_by_batch(
        executor: &mut sqlx::postgres::PgConnection,
        batch_id: i64,
    ) -> Result<Vec<i64>> {
        let ids: Vec<i64> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT i.operation_id
            FROM material_requisition_items i
            JOIN material_requisitions r ON r.id = i.requisition_id
            WHERE i.batch_id = $1
              AND i.operation_id IS NOT NULL
              AND r.deleted_at IS NULL
              AND r.status <> $2
            "#,
        )
        .bind(batch_id)
        .bind(RequisitionStatus::Cancelled)
        .fetch_all(executor)
        .await?;
        Ok(ids)
    }
}
