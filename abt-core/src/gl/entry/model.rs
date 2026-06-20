use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use super::super::enums::EntryStatus;
use crate::shared::enums::document_type::DocumentType;

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// 凭证表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GlEntry {
    pub id: i64,
    pub doc_number: String,
    pub period: String,
    pub entry_date: NaiveDate,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub description: String,
    pub voucher_type: String,
    pub is_opening: bool,
    pub status: EntryStatus,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub operator_id: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 凭证分录行实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GlEntryLine {
    pub id: i64,
    pub entry_id: i64,
    pub account_id: i64,
    pub debit: Decimal,
    pub credit: Decimal,
    pub amount_currency: Decimal,
    pub currency: String,
    pub exchange_rate: Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub project_id: Option<i64>,
    pub memo: String,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 凭证分录行输入
#[derive(Debug, Clone)]
pub struct GlEntryLineInput {
    pub account_id: i64,
    pub debit: Decimal,
    pub credit: Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub project_id: Option<i64>,
    pub memo: String,
}

/// 创建手工凭证请求
#[derive(Debug, Clone)]
pub struct CreateManualEntryReq {
    pub entry_date: NaiveDate,
    pub description: String,
    pub voucher_type: String,
    pub is_opening: bool,
    pub lines: Vec<GlEntryLineInput>,
}

/// 凭证查询过滤
#[derive(Debug, Clone, Default)]
pub struct GlEntryFilter {
    pub period: Option<String>,
    pub source_type: Option<DocumentType>,
    pub status: Option<EntryStatus>,
    pub voucher_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// 试算平衡表行
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TrialBalanceRow {
    pub account_id: i64,
    pub code: String,
    pub name: String,
    pub account_type: i16,
    pub balance_direction: i16,
    /// 本期期初余额（静态 opening_balance + 本期之前 posted 累计）
    pub opening_balance: Decimal,
    pub period_debit: Decimal,
    pub period_credit: Decimal,
    pub end_balance: Decimal,
}

/// 试算平衡表
#[derive(Debug, Clone)]
pub struct TrialBalance {
    pub rows: Vec<TrialBalanceRow>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
}

/// 明细账行（总账明细）
#[derive(Debug, Clone)]
pub struct GlDetailRow {
    pub entry_id: i64,
    pub doc_number: String,
    pub entry_date: NaiveDate,
    pub memo: String,
    pub counterpart_account_id: Option<i64>,
    pub debit: Decimal,
    pub credit: Decimal,
    pub running_balance: Decimal,
}
