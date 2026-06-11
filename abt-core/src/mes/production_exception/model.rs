use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::super::enums::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductionException {
    pub id: i64,
    pub doc_number: String,
    pub exception_type: ExceptionType,
    pub status: ExceptionStatus,
    pub severity: ExceptionSeverity,
    pub reason_category: Option<ReasonCategory>,
    pub work_order_id: Option<i64>,
    pub batch_id: Option<i64>,
    pub product_id: Option<i64>,
    pub current_step: Option<i32>,
    pub impact_qty: Option<Decimal>,
    pub description: Option<String>,
    pub disposition: Option<String>,
    pub found_at: DateTime<Utc>,
    pub finder_id: Option<i64>,
    pub owner_id: Option<i64>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 新建异常参数（insert 参数封装）
pub struct CreateExceptionParams<'a> {
    pub doc_number: &'a str,
    pub exception_type: ExceptionType,
    pub severity: ExceptionSeverity,
    pub reason_category: Option<ReasonCategory>,
    pub work_order_id: Option<i64>,
    pub batch_id: Option<i64>,
    pub product_id: Option<i64>,
    pub current_step: Option<i32>,
    pub impact_qty: Option<Decimal>,
    pub description: Option<&'a str>,
    pub disposition: Option<&'a str>,
    pub found_at: DateTime<Utc>,
    pub finder_id: Option<i64>,
    pub owner_id: Option<i64>,
    pub operator_id: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExceptionEvent {
    pub id: i64,
    pub exception_id: i64,
    pub event_type: String,
    pub description: Option<String>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

/// 列表视图（关联名称）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExceptionListItem {
    pub id: i64,
    pub doc_number: String,
    pub exception_type: ExceptionType,
    pub status: ExceptionStatus,
    pub reason_category: Option<ReasonCategory>,
    pub work_order_id: Option<i64>,
    pub batch_id: Option<i64>,
    pub wo_doc_number: Option<String>,
    pub batch_no: Option<String>,
    pub product_name: Option<String>,
    pub description: Option<String>,
    pub impact_qty: Option<Decimal>,
    pub found_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct ExceptionListFilter {
    pub exception_type: Option<ExceptionType>,
    pub status: Option<ExceptionStatus>,
    pub reason_category: Option<ReasonCategory>,
    pub keyword: Option<String>,
    pub date_from: Option<chrono::NaiveDate>,
    pub date_to: Option<chrono::NaiveDate>,
}

/// 异常统计
#[derive(Debug, Clone)]
pub struct ExceptionStats {
    pub total_month: i64,
    pub batch_suspended: i64,
    pub batch_scrapped: i64,
    pub inspection_failed: i64,
}

/// 详情 lookups
#[derive(Debug, Clone)]
pub struct ExceptionDetailLookups {
    pub wo_doc_number: Option<String>,
    pub batch_no: Option<String>,
    pub product_name: Option<String>,
    pub finder_name: Option<String>,
    pub owner_name: Option<String>,
}
