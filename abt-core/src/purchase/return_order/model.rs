use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::PurchaseReturnStatus;

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

/// 采购退货主表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseReturn {
    pub id: i64,
    pub doc_number: String,
    pub order_id: i64,
    pub supplier_id: i64,
    pub return_date: NaiveDate,
    pub status: PurchaseReturnStatus,
    pub return_reason: String,
    pub total_amount: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购退货明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseReturnItem {
    pub id: i64,
    pub return_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub returned_qty: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 采购退货查询条件
#[derive(Debug, Clone, Default)]
pub struct PurchaseReturnQuery {
    pub order_id: Option<i64>,
    pub supplier_id: Option<i64>,
    pub status: Option<PurchaseReturnStatus>,
    pub return_date_start: Option<chrono::NaiveDate>,
    pub return_date_end: Option<chrono::NaiveDate>,
}

// ---------------------------------------------------------------------------
// Create request structs
// ---------------------------------------------------------------------------

/// 创建采购退货请求
pub struct CreatePurchaseReturnRequest {
    pub order_id: i64,
    pub supplier_id: i64,
    pub return_date: NaiveDate,
    pub return_reason: String,
    pub remark: String,
    pub items: Vec<CreateReturnItemRequest>,
}

/// 创建退货明细请求
pub struct CreateReturnItemRequest {
    pub order_item_id: i64,
    pub product_id: i64,
    pub returned_qty: Decimal,
    pub unit_price: Decimal,
}
