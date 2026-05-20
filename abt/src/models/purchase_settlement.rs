//! 采购结算数据模型
//!
//! 包含采购对账单、采购发票、采购付款实体及查询参数。

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ============================================================================
// PurchaseStatement
// ============================================================================

/// 采购对账单实体
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseStatement {
    pub statement_id: i64,
    pub statement_no: String,
    pub supplier_id: i64,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_amount: Decimal,
    /// 1=待确认, 2=已确认, 3=有异议
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 对账单行项目
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct StatementItem {
    pub item_id: i64,
    pub statement_id: i64,
    pub po_id: i64,
    pub po_no: Option<String>,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}

/// 对账单详情（含供应商名称，用于列表查询）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatementDetail {
    pub statement_id: i64,
    pub statement_no: String,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_amount: Decimal,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 对账单查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StatementQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<i16>,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 对账单详情（含行项目）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatementWithItems {
    #[serde(flatten)]
    pub statement: PurchaseStatement,
    pub items: Vec<StatementItem>,
}

// ============================================================================
// PurchaseInvoice
// ============================================================================

/// 采购发票实体
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchaseInvoice {
    pub invoice_id: i64,
    pub invoice_no: String,
    pub supplier_id: i64,
    pub statement_id: Option<i64>,
    pub invoice_amount: Decimal,
    pub invoice_date: NaiveDate,
    /// 1=已登记, 2=已核验
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// 发票详情（含供应商名称和对账单编号，用于列表查询）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InvoiceDetail {
    pub invoice_id: i64,
    pub invoice_no: String,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub statement_id: Option<i64>,
    pub statement_no: Option<String>,
    pub invoice_amount: Decimal,
    pub invoice_date: NaiveDate,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// 发票查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InvoiceQuery {
    pub supplier_id: Option<i64>,
    pub statement_id: Option<i64>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

// ============================================================================
// PurchasePayment
// ============================================================================

/// 采购付款实体
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct PurchasePayment {
    pub payment_id: i64,
    pub payment_no: String,
    pub supplier_id: i64,
    pub invoice_id: Option<i64>,
    pub payment_amount: Decimal,
    pub payment_method: Option<String>,
    /// 1=待审批, 2=已审批, 3=已付款
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 付款详情（含供应商名称和发票编号，用于列表查询）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentDetail {
    pub payment_id: i64,
    pub payment_no: String,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub invoice_id: Option<i64>,
    pub invoice_no: Option<String>,
    pub payment_amount: Decimal,
    pub payment_method: Option<String>,
    pub status: i16,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 付款查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PaymentQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<i16>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
