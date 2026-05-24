use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::BackflushStatus;

/// 冲扣记录实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BackflushRecord {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub product_id: i64,
    pub completed_qty: Decimal,
    pub backflush_date: NaiveDate,
    pub status: BackflushStatus,
    pub variance_threshold: Decimal,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 冲扣明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BackflushItem {
    pub id: i64,
    pub record_id: i64,
    pub component_id: i64,
    pub theoretical_qty: Decimal,
    pub actual_qty: Decimal,
    pub variance_qty: Decimal,
    pub variance_rate: Decimal,
    pub is_over_threshold: bool,
}

/// 创建冲扣记录请求
#[derive(Debug, Clone)]
pub struct CreateBackflushReq {
    pub doc_number: String,
    pub work_order_id: i64,
    pub product_id: i64,
    pub completed_qty: Decimal,
    pub backflush_date: NaiveDate,
    pub variance_threshold: Decimal,
    pub operator_id: i64,
}

/// 创建冲扣明细请求
#[derive(Debug, Clone)]
pub struct CreateBackflushItemReq {
    pub record_id: i64,
    pub component_id: i64,
    pub theoretical_qty: Decimal,
    pub actual_qty: Decimal,
    pub variance_qty: Decimal,
    pub variance_rate: Decimal,
    pub is_over_threshold: bool,
}

/// 冲扣记录查询过滤
#[derive(Debug, Clone, Default)]
pub struct BackflushFilter {
    pub status: Option<BackflushStatus>,
    pub work_order_id: Option<i64>,
}
