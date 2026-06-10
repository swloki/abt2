use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::super::enums::TransactionType;

/// 库存事务实体（Append-only）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryTransaction {
    pub id: i64,
    pub doc_number: Option<String>,
    pub transaction_type: TransactionType,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub batch_no: Option<String>,
    pub quantity: Decimal,
    pub unit_cost: Option<Decimal>,
    pub source_type: String,
    pub source_id: i64,
    pub remark: Option<String>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

/// 记录事务请求
#[derive(Debug, Clone)]
pub struct RecordTransactionReq {
    pub doc_number: Option<String>,
    pub transaction_type: TransactionType,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub batch_no: Option<String>,
    pub quantity: Decimal,
    pub unit_cost: Option<Decimal>,
    pub source_type: String,
    pub source_id: i64,
    pub remark: Option<String>,
}

/// 事务查询过滤
#[derive(Debug, Clone, Default)]
pub struct TransactionFilter {
    pub transaction_type: Option<TransactionType>,
    pub product_id: Option<i64>,
    pub warehouse_id: Option<i64>,
    pub source_type: Option<String>,
    pub source_id: Option<i64>,
    pub doc_number: Option<String>,
    pub product_code: Option<String>,
}
