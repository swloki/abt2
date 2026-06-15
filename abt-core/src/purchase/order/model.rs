use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::PurchaseOrderStatus;

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

/// 采购订单主表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseOrder {
    pub id: i64,
    pub doc_number: String,
    pub supplier_id: i64,
    pub order_date: NaiveDate,
    pub expected_delivery_date: Option<NaiveDate>,
    pub status: PurchaseOrderStatus,
    pub total_amount: Decimal,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购订单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseOrderItem {
    pub id: i64,
    pub order_id: i64,
    pub line_no: i32,
    pub product_id: i64,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub received_qty: Decimal,
    pub inspected_qty: Decimal,
    pub returned_qty: Decimal,
    pub quotation_item_id: Option<i64>,
    pub expected_delivery_date: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 采购订单查询条件
#[derive(Debug, Clone, Default)]
pub struct PurchaseOrderQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<PurchaseOrderStatus>,
    pub order_date_start: Option<NaiveDate>,
    pub order_date_end: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// Create request structs
// ---------------------------------------------------------------------------

/// 创建采购订单请求
pub struct CreatePurchaseOrderRequest {
    pub supplier_id: i64,
    pub order_date: NaiveDate,
    pub expected_delivery_date: Option<NaiveDate>,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: String,
    pub items: Vec<CreateOrderItemRequest>,
}

/// 创建订单明细请求
#[derive(Debug, Clone)]
pub struct CreateOrderItemRequest {
    pub product_id: i64,
    pub line_no: i32,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub quotation_item_id: Option<i64>,
    pub expected_delivery_date: Option<NaiveDate>,
}

/// 更新采购订单请求（仅草稿可编辑）
pub struct UpdatePurchaseOrderRequest {
    pub supplier_id: i64,
    pub expected_delivery_date: Option<NaiveDate>,
    pub payment_terms: Option<String>,
    pub delivery_address: Option<String>,
    pub remark: String,
}
