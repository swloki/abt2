use crate::shared::types::Result;

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
}
