use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::PurchaseReconStatus;

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

/// 对账单主表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseReconciliation {
    pub id: i64,
    pub doc_number: String,
    pub supplier_id: i64,
    pub period: String,
    pub status: PurchaseReconStatus,
    pub total_amount: Decimal,
    pub confirmed_amount: Decimal,
    pub difference: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 对账单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseReconItem {
    pub id: i64,
    pub reconciliation_id: i64,
    pub order_id: i64,
    pub order_item_id: i64,
    pub received_qty: Decimal,
    pub returned_qty: Decimal,
    pub returned_amount: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub confirmed: bool,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 对账单查询条件
#[derive(Debug, Clone, Default)]
pub struct PurchaseReconciliationQuery {
    pub supplier_id: Option<i64>,
    pub period: Option<String>,
    pub status: Option<PurchaseReconStatus>,
}

// ---------------------------------------------------------------------------
// Preview DTO（创建页只读预览，口径与 create 落库一致）
// ---------------------------------------------------------------------------

/// 待对账明细预览项（某供应商某期间内「未对账已收货」的订单明细）
#[derive(Debug, Clone)]
pub struct PurchaseReconPreviewItem {
    pub order_id: i64,
    pub order_item_id: i64,
    pub product_id: i64,
    pub received_qty: Decimal,
    pub returned_qty: Decimal,
    pub returned_amount: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}
