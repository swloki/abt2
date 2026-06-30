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
    pub warehouse_id: Option<i64>,
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
    pub warehouse_id: Option<i64>,
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
    pub warehouse_id: Option<i64>,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub receipt_date: NaiveDate,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReceiptDetailLookups {
    pub wo_doc_number: Option<String>,
    pub batch_no: Option<String>,
    pub product_name: Option<String>,
    pub warehouse_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReceiptListFilter {
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReceiptListItem {
    pub id: i64,
    pub doc_number: String,
    pub work_order_doc: Option<String>,
    pub batch_id: Option<i64>,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub received_qty: rust_decimal::Decimal,
    pub warehouse_name: Option<String>,
    pub status: i16,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// FQC 门控状态（供 UI 查询，不触发 confirm）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FqcGate {
    NotRequired,
    PendingInspection,
    AllPassed,
    HasFailed,
}
