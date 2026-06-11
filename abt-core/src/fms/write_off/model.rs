use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::WriteOffType;
use crate::shared::enums::document_type::DocumentType;

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// 核销记录主实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WriteOff {
    pub id: i64,
    pub write_off_type: WriteOffType,
    pub cash_journal_id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub amount: Decimal,
    pub write_off_date: NaiveDate,
    pub idempotency_key: Option<String>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 创建核销请求
#[derive(Debug, Clone)]
pub struct WriteOffReq {
    pub cash_journal_id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_total: Decimal,
    pub amount: Decimal,
    pub idempotency_key: Option<String>,
}

// ---------------------------------------------------------------------------
// List Filter
// ---------------------------------------------------------------------------

/// 核销列表查询筛选条件
#[derive(Debug, Clone, Default)]
pub struct WriteOffListFilter {
    pub write_off_type: Option<WriteOffType>,
    pub keyword: Option<String>,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
}
