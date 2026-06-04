use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::{PaymentMethod, PaymentStatus};

// ---------------------------------------------------------------------------
// Entity struct
// ---------------------------------------------------------------------------

/// 付款申请实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaymentRequest {
    pub id: i64,
    pub doc_number: String,
    pub supplier_id: i64,
    pub reconciliation_id: Option<i64>,
    pub payment_date: NaiveDate,
    pub amount: Decimal,
    pub status: PaymentStatus,
    pub payment_method: PaymentMethod,
    pub bank_account_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub invoice_amount: Option<Decimal>,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 付款申请查询条件
#[derive(Debug, Clone, Default)]
pub struct PaymentRequestQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<PaymentStatus>,
    pub payment_date_start: Option<NaiveDate>,
    pub payment_date_end: Option<NaiveDate>,
    pub keyword: Option<String>,
    pub payment_method: Option<PaymentMethod>,
}

// ---------------------------------------------------------------------------
// Create request struct
// ---------------------------------------------------------------------------

/// 创建付款申请请求
pub struct CreatePaymentRequestRequest {
    pub supplier_id: i64,
    pub reconciliation_id: Option<i64>,
    pub payment_date: NaiveDate,
    pub amount: Decimal,
    pub payment_method: PaymentMethod,
    pub bank_account_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub invoice_amount: Option<Decimal>,
    pub remark: String,
}
