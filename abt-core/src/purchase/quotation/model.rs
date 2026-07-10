use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::PurchaseQuotationStatus;

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

/// 采购报价主表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseQuotation {
    pub id: i64,
    pub doc_number: String,
    pub supplier_id: i64,
    pub quotation_date: NaiveDate,
    pub valid_from: NaiveDate,
    pub valid_until: NaiveDate,
    pub status: PurchaseQuotationStatus,
    pub remark: String,
    pub operator_id: i64,
    pub currency: String,
    pub buyer_id: Option<i64>,
    pub supplier_quotation_no: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 采购报价明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseQuotationItem {
    pub id: i64,
    pub quotation_id: i64,
    pub product_id: i64,
    pub line_no: i32,
    pub unit_price: Decimal,
    pub min_order_qty: Option<Decimal>,
    pub lead_time_days: Option<i32>,
    pub currency: String,
    pub is_preferred: bool,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 采购报价查询条件
#[derive(Debug, Clone, Default)]
pub struct PurchaseQuotationQuery {
    pub supplier_id: Option<i64>,
    pub status: Option<PurchaseQuotationStatus>,
    pub quotation_date_start: Option<NaiveDate>,
    pub quotation_date_end: Option<NaiveDate>,
    /// 单号模糊匹配（ILIKE '%kw%'）
    pub doc_number: Option<String>,
    /// 物料关键字反查（EXISTS join items+products，product_code OR pdt_name）
    pub product_keyword: Option<String>,
    /// 排序列（白名单：date/valid/supplier/doc）
    pub sort: Option<String>,
    /// 排序方向（"asc" | "desc"，默认 desc）
    pub dir: Option<String>,
}

// ---------------------------------------------------------------------------
// Create request structs
// ---------------------------------------------------------------------------

/// 创建采购报价请求
pub struct CreatePurchaseQuotationRequest {
    pub supplier_id: i64,
    pub quotation_date: NaiveDate,
    pub valid_from: NaiveDate,
    pub valid_until: NaiveDate,
    pub remark: String,
    pub currency: String,
    pub buyer_id: Option<i64>,
    pub supplier_quotation_no: String,
    pub items: Vec<CreateQuotationItemRequest>,
}

/// 创建报价明细请求
pub struct CreateQuotationItemRequest {
    pub product_id: i64,
    pub line_no: i32,
    pub unit_price: Decimal,
    pub min_order_qty: Option<Decimal>,
    pub lead_time_days: Option<i32>,
    pub currency: String,
    pub is_preferred: bool,
}

// ---------------------------------------------------------------------------
// Comparison view
// ---------------------------------------------------------------------------

/// 供应商报价对比（按产品维度）
#[derive(Debug, Clone)]
pub struct QuotationComparison {
    pub product_id: i64,
    pub supplier_id: i64,
    pub unit_price: Decimal,
    pub currency: String,
    pub valid_until: NaiveDate,
    pub is_preferred: bool,
}
