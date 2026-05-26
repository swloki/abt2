use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::shared::types::RepoResult;

use super::model::*;
use super::super::enums::*;

pub struct ProductionReceiptRepo;

impl ProductionReceiptRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        work_order_id: i64,
        batch_id: Option<i64>,
        product_id: i64,
        received_qty: Decimal,
        warehouse_id: i64,
        zone_id: Option<i64>,
        bin_id: Option<i64>,
        receipt_date: NaiveDate,
        doc_number: &str,
        operator_id: i64,
    ) -> RepoResult<ProductionReceipt> {
        let row = sqlx::query(
            r#"
            INSERT INTO production_receipts
                (doc_number, work_order_id, batch_id, product_id,
                 received_qty, warehouse_id, zone_id, bin_id,
                 receipt_date, status, backflush_triggered, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, false, '', $11)
            RETURNING id, doc_number, work_order_id, batch_id, product_id,
                      received_qty, warehouse_id, zone_id, bin_id,
                      receipt_date, status, backflush_triggered, remark, operator_id,
                      created_at, updated_at
            "#,
        )
        .bind(doc_number)
        .bind(work_order_id)
        .bind(batch_id)
        .bind(product_id)
        .bind(received_qty)
        .bind(warehouse_id)
        .bind(zone_id)
        .bind(bin_id)
        .bind(receipt_date)
        .bind(ReceiptStatus::Draft)
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(ProductionReceipt::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> RepoResult<Option<ProductionReceipt>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, batch_id, product_id,
                   received_qty, warehouse_id, zone_id, bin_id,
                   receipt_date, status, backflush_triggered, remark, operator_id,
                   created_at, updated_at
            FROM production_receipts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ProductionReceipt::from_row(&r).map_err(Into::into)).transpose()

    }

    pub async fn update_status(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: ReceiptStatus,
    ) -> RepoResult<bool> {
        let result = sqlx::query(
            r#"
            UPDATE production_receipts
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn set_backflush_triggered(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        value: bool,
    ) -> RepoResult<bool> {
        let result = sqlx::query(
            r#"
            UPDATE production_receipts
            SET backflush_triggered = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(value)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
