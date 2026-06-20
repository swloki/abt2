use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use super::super::invoice::InvoiceStatus;

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// 采购发票表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseInvoice {
    pub id: i64,
    pub doc_number: String,
    pub supplier_id: i64,
    pub issue_date: NaiveDate,
    pub period: String,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
    pub status: InvoiceStatus,
    pub source_arrival_id: Option<i64>,
    pub gl_entry_id: Option<i64>,
    pub operator_id: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购发票行实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseInvoiceItem {
    pub id: i64,
    pub invoice_id: i64,
    pub product_id: i64,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub tax_rate_id: Option<i64>,
    pub line_subtotal: Decimal,
    pub line_tax: Decimal,
    pub line_total: Decimal,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 创建采购发票请求
#[derive(Debug, Clone)]
pub struct CreatePurchaseInvoiceReq {
    pub supplier_id: i64,
    pub issue_date: NaiveDate,
    pub items: Vec<PurchaseInvoiceItemInput>,
    pub source_arrival_id: Option<i64>,
}

/// 采购发票行输入
#[derive(Debug, Clone)]
pub struct PurchaseInvoiceItemInput {
    pub product_id: i64,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub tax_rate_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Query filter
// ---------------------------------------------------------------------------

/// 采购发票查询过滤
#[derive(Debug, Clone, Default)]
pub struct PurchaseInvoiceFilter {
    pub supplier_id: Option<i64>,
    pub status: Option<InvoiceStatus>,
    pub period: Option<String>,
}
