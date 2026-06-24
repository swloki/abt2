use crate::shared::types::{PaginatedResult, Result};

use super::model::*;
use super::super::enums::*;

pub struct ProductionReceiptRepo;

impl ProductionReceiptRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        params: &InsertReceiptParams<'_>,
    ) -> Result<ProductionReceipt> {
        let row = sqlx::query!(
            r#"
            INSERT INTO production_receipts
                (doc_number, work_order_id, batch_id, product_id,
                 received_qty, warehouse_id, zone_id, bin_id,
                 receipt_date, status, backflush_triggered, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, false, '', $11)
            RETURNING id, doc_number, work_order_id, batch_id, product_id,
                      received_qty, warehouse_id, zone_id, bin_id,
                      receipt_date, status as "status: i16", backflush_triggered, remark, operator_id,
                      created_at, updated_at
            "#,
            params.doc_number,
            params.work_order_id,
            params.batch_id,
            params.product_id,
            params.received_qty,
            params.warehouse_id,
            params.zone_id,
            params.bin_id,
            params.receipt_date,
            ReceiptStatus::Draft.as_i16(),
            params.operator_id,
        )
        .fetch_one(&mut *executor)
        .await?;

        Ok(ProductionReceipt {
            id: row.id,
            doc_number: row.doc_number,
            work_order_id: row.work_order_id,
            batch_id: row.batch_id,
            product_id: row.product_id,
            received_qty: row.received_qty,
            warehouse_id: row.warehouse_id,
            zone_id: row.zone_id,
            bin_id: row.bin_id,
            receipt_date: row.receipt_date,
            status: ReceiptStatus::from_i16(row.status).unwrap_or(ReceiptStatus::Draft),
            backflush_triggered: row.backflush_triggered,
            remark: row.remark,
            operator_id: row.operator_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<ProductionReceipt>> {
        let row = sqlx::query!(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, product_id,
                   received_qty, warehouse_id, zone_id, bin_id,
                   receipt_date, status as "status: i16", backflush_triggered, remark, operator_id,
                   created_at, updated_at
            FROM production_receipts
            WHERE id = $1
            "#,
            id,
        )
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| {
            Ok(ProductionReceipt {
                id: r.id,
                doc_number: r.doc_number,
                work_order_id: r.work_order_id,
                batch_id: r.batch_id,
                product_id: r.product_id,
                received_qty: r.received_qty,
                warehouse_id: r.warehouse_id,
                zone_id: r.zone_id,
                bin_id: r.bin_id,
                receipt_date: r.receipt_date,
                status: ReceiptStatus::from_i16(r.status).unwrap_or(ReceiptStatus::Draft),
                backflush_triggered: r.backflush_triggered,
                remark: r.remark,
                operator_id: r.operator_id,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
        })
        .transpose()
    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: ReceiptStatus,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE production_receipts
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            status.as_i16(),
        )
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn set_backflush_triggered(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        value: bool,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE production_receipts
            SET backflush_triggered = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            value,
        )
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &ReceiptListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReceiptListItem>> {
        let offset = page.saturating_sub(1) * page_size;

        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 0u32;

        if filter.keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("r.doc_number ILIKE ${param_idx}"));
        }

        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;

        let count_sql = format!(
            "SELECT COUNT(*) FROM production_receipts r WHERE {where_sql}"
        );
        let data_sql = format!(
            "SELECT r.id, r.doc_number, wo.doc_number AS work_order_doc, \
             r.batch_id, r.product_id, p.pdt_name AS product_name, \
             r.received_qty, w.name AS warehouse_name, r.status, r.created_at \
             FROM production_receipts r \
             LEFT JOIN work_orders wo ON r.work_order_id = wo.id \
             LEFT JOIN products p ON r.product_id = p.product_id \
             LEFT JOIN warehouses w ON r.warehouse_id = w.id \
             WHERE {where_sql} \
             ORDER BY r.created_at DESC \
             LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query_as::<_, ReceiptListItem>(sqlx::AssertSqlSafe(data_sql));

        if let Some(ref kw) = filter.keyword {
            let pattern = format!("%{kw}%");
            count_q = count_q.bind(pattern.clone());
            data_q = data_q.bind(pattern);
        }

        data_q = data_q.bind(page_size as i64).bind(offset as i64);

        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let items = data_q.fetch_all(&mut *executor).await?;

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

    /// 按工单 ID 查所有入库单（工作台用，复用 list 的 JOIN）
    pub async fn list_by_work_order(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
    ) -> Result<Vec<ReceiptListItem>> {
        let data_sql = "SELECT r.id, r.doc_number, wo.doc_number AS work_order_doc, \
             r.batch_id, r.product_id, p.pdt_name AS product_name, \
             r.received_qty, w.name AS warehouse_name, r.status, r.created_at \
             FROM production_receipts r \
             LEFT JOIN work_orders wo ON r.work_order_id = wo.id \
             LEFT JOIN products p ON r.product_id = p.product_id \
             LEFT JOIN warehouses w ON r.warehouse_id = w.id \
             WHERE r.work_order_id = $1 AND r.deleted_at IS NULL \
             ORDER BY r.created_at DESC";
        let items = sqlx::query_as::<_, ReceiptListItem>(sqlx::AssertSqlSafe(
            data_sql.to_string(),
        ))
        .bind(work_order_id)
        .fetch_all(&mut *executor)
        .await?;
        Ok(items)
    }
}
