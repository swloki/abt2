use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::super::enums::TransactionType;

// ── 写操作结果（映射 proto InventoryLogResponse）──

#[derive(Debug, Clone)]
pub struct StockOperationResult {
    pub transaction_id: i64,
    pub stock_ledger_id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub zone_id: i64,
    pub bin_id: i64,
    pub before_qty: Decimal,
    pub after_qty: Decimal,
    pub change_qty: Decimal,
}

// ── 库存详情视图（映射 proto InventoryDetailResponse）──

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryDetailView {
    pub stock_ledger_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub bin_id: i64,
    pub bin_code: String,
    pub quantity: Decimal,
    pub safety_stock: Decimal,
}

// ── 事务日志详情视图（映射 proto InventoryLogDetailResponse）──

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TransactionDetailView {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub bin_id: i64,
    pub bin_code: String,
    pub transaction_type: TransactionType,
    pub quantity: Decimal,
    pub source_type: String,
    pub source_id: i64,
    pub remark: Option<String>,
    pub operator_id: i64,
    pub operator_name: String,
    pub created_at: DateTime<Utc>,
}

// ── 请求类型 ──

#[derive(Debug, Clone)]
pub struct StockChangeReq {
    pub product_id: i64,
    pub warehouse_id: i64,
    pub zone_id: i64,
    pub bin_id: i64,
    pub quantity: Decimal,
    pub ref_order_type: Option<String>,
    pub ref_order_id: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StockTransferReq {
    pub product_id: i64,
    pub from_warehouse_id: i64,
    pub from_zone_id: i64,
    pub from_bin_id: i64,
    pub to_warehouse_id: i64,
    pub to_zone_id: i64,
    pub to_bin_id: i64,
    pub quantity: Decimal,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InventoryQueryFilter {
    pub product_id: Option<i64>,
    pub keyword: Option<String>,
    pub warehouse_id: Option<i64>,
    pub bin_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct TransactionLogFilter {
    pub product_id: Option<i64>,
    pub product_name: Option<String>,
    pub product_code: Option<String>,
    pub bin_id: Option<i64>,
    pub warehouse_id: Option<i64>,
    pub transaction_type: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}
