use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::super::enums::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductionReceipt {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub batch_id: Option<i64>,
    pub product_id: i64,
    pub received_qty: Decimal,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub receipt_date: NaiveDate,
    pub status: ReceiptStatus,
    pub backflush_triggered: bool,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 入库单插入参数
pub struct InsertReceiptParams<'a> {
    pub work_order_id: i64,
    pub batch_id: Option<i64>,
    pub product_id: i64,
    pub received_qty: Decimal,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub receipt_date: NaiveDate,
    pub doc_number: &'a str,
    pub operator_id: i64,
}

#[derive(Debug, Clone)]
pub struct CreateReceiptReq {
    pub work_order_id: i64,
    pub batch_id: Option<i64>,
    pub product_id: i64,
    pub received_qty: Decimal,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub receipt_date: NaiveDate,
    pub remark: Option<String>,
}
