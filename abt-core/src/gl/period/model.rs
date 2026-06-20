use chrono::{DateTime, NaiveDate, Utc};
use super::super::enums::PeriodStatus;

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// 会计期间表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountingPeriod {
    pub id: i64,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: PeriodStatus,
    pub fiscal_year: String,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_by: Option<i64>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Query filter
// ---------------------------------------------------------------------------

/// 期间查询过滤
#[derive(Debug, Clone, Default)]
pub struct PeriodFilter {
    pub fiscal_year: Option<String>,
    pub status: Option<PeriodStatus>,
}
