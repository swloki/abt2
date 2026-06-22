use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use super::super::invoice::InvoiceStatus;

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// 销售发票表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesInvoice {
    pub id: i64,
    pub doc_number: String,
    pub customer_id: i64,
    pub issue_date: NaiveDate,
    pub period: String,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
    pub status: InvoiceStatus,
    pub source_shipping_id: Option<i64>,
    pub gl_entry_id: Option<i64>,
    pub due_date: Option<NaiveDate>,
    pub outstanding_amount: Decimal,
    pub paid_amount: Decimal,
    pub is_return: bool,
    pub return_against: Option<i64>,
    pub operator_id: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 销售发票行实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SalesInvoiceItem {
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

/// 创建销售发票请求
#[derive(Debug, Clone)]
pub struct CreateSalesInvoiceReq {
    pub customer_id: i64,
    pub issue_date: NaiveDate,
    pub items: Vec<SalesInvoiceItemInput>,
    pub source_shipping_id: Option<i64>,
}

/// 销售发票行输入
#[derive(Debug, Clone)]
pub struct SalesInvoiceItemInput {
    pub product_id: i64,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub tax_rate_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Query filter
// ---------------------------------------------------------------------------

/// 销售发票查询过滤
#[derive(Debug, Clone, Default)]
pub struct SalesInvoiceFilter {
    pub customer_id: Option<i64>,
    pub status: Option<InvoiceStatus>,
    pub period: Option<String>,
}
