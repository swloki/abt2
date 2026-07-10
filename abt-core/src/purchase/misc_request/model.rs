use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::MiscRequestStatus;

// ---------------------------------------------------------------------------
// Entity structs
// ---------------------------------------------------------------------------

/// 零星请购主表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MiscellaneousRequest {
    pub id: i64,
    pub doc_number: String,
    pub department_id: i64,
    pub request_date: NaiveDate,
    pub status: MiscRequestStatus,
    pub total_amount: Decimal,
    pub purpose: String,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 零星请购明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MiscRequestItem {
    pub id: i64,
    pub request_id: i64,
    pub line_no: i32,
    pub item_name: String,
    pub specification: Option<String>,
    pub quantity: Decimal,
    pub unit: String,
    pub estimated_price: Option<Decimal>,
    pub remark: Option<String>,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

/// 零星请购查询条件
#[derive(Debug, Clone, Default)]
pub struct MiscRequestQuery {
    pub department_id: Option<i64>,
    pub status: Option<MiscRequestStatus>,
    pub request_date_start: Option<NaiveDate>,
    pub request_date_end: Option<NaiveDate>,
    /// 单号模糊匹配（ILIKE '%kw%'）
    pub doc_number: Option<String>,
    /// 物品关键字反查（EXISTS join misc_request_items，item_name OR specification）
    pub item_keyword: Option<String>,
    /// 排序列（白名单：date/amount/purpose/doc）
    pub sort: Option<String>,
    /// 排序方向（"asc" | "desc"，默认 desc）
    pub dir: Option<String>,
}

// ---------------------------------------------------------------------------
// Create request structs
// ---------------------------------------------------------------------------

/// 创建零星请购请求
pub struct CreateMiscRequestRequest {
    pub department_id: i64,
    pub request_date: NaiveDate,
    pub purpose: String,
    pub remark: String,
    pub items: Vec<CreateMiscItemRequest>,
}

/// 创建零星请购明细请求
pub struct CreateMiscItemRequest {
    pub line_no: i32,
    pub item_name: String,
    pub specification: Option<String>,
    pub quantity: Decimal,
    pub unit: String,
    pub estimated_price: Option<Decimal>,
    pub remark: Option<String>,
}
