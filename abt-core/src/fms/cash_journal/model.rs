use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::{
    CashDirection, CounterpartyRef, CounterpartyType, JournalStatus, JournalType,
};
use crate::shared::enums::document_type::DocumentType;

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// 出纳日记账主实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CashJournal {
    pub id: i64,
    pub doc_number: String,
    pub journal_type: JournalType,
    pub direction: CashDirection,
    pub amount: Decimal,
    pub counterparty_type: CounterpartyType,
    pub counterparty_id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub bank_account: String,
    pub transaction_date: NaiveDate,
    pub period: String,
    pub status: JournalStatus,
    pub remark: String,
    pub operator_id: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl CashJournal {
    /// Reconstruct CounterpartyRef from the two DB columns
    pub fn counterparty(&self) -> CounterpartyRef {
        CounterpartyRef::from_parts(self.counterparty_type, self.counterparty_id)
    }
}

/// 日记账明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CashJournalLine {
    pub id: i64,
    pub journal_id: i64,
    pub account_code: String,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub remark: String,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 创建出纳日记账请求
#[derive(Debug, Clone)]
pub struct CreateCashJournalReq {
    pub journal_type: JournalType,
    pub direction: CashDirection,
    pub amount: Decimal,
    pub counterparty: CounterpartyRef,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub bank_account: String,
    pub transaction_date: NaiveDate,
    pub period: String,
    pub remark: String,
    pub lines: Vec<CashJournalLineInput>,
}

/// 创建日记账明细输入
#[derive(Debug, Clone)]
pub struct CashJournalLineInput {
    pub account_code: String,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub remark: String,
}

// ---------------------------------------------------------------------------
// Query filter
// ---------------------------------------------------------------------------

/// 出纳日记账查询过滤
#[derive(Debug, Clone, Default)]
pub struct CashJournalFilter {
    pub period: Option<String>,
    pub journal_type: Option<JournalType>,
    pub status: Vec<JournalStatus>,
    pub counterparty_id: Option<i64>,
    pub transaction_date_from: Option<NaiveDate>,
    pub transaction_date_to: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// 期间收支汇总
#[derive(Debug, Clone)]
pub struct BalanceSummary {
    pub total_inflow: Decimal,
    pub total_outflow: Decimal,
    pub net_balance: Decimal,
    pub currency: String,
}

// ---------------------------------------------------------------------------
// Search / Picker types
// ---------------------------------------------------------------------------

/// 往来方搜索结果（用于 Entity Picker）
#[derive(Debug, Clone, serde::Serialize)]
pub struct CounterpartyResult {
    pub id: i64,
    pub name: String,
    pub code: String,
}

/// 会计科目搜索结果（用于分录行科目选择）
#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountResult {
    pub id: i64,
    pub code: String,
    pub name: String,
}
