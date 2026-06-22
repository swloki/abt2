use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::enums::{AgeingBasis, LedgerDirection};
use crate::fms::enums::CounterpartyType;
use crate::shared::enums::document_type::DocumentType;

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// 应收应付台账实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArApLedger {
    pub id: i64,
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub account_id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_doc_no: String,
    pub against_type: Option<DocumentType>,
    pub against_id: Option<i64>,
    pub direction: LedgerDirection,
    pub amount: Decimal,
    pub amount_applied: Decimal,
    pub currency: String,
    pub exchange_rate: Decimal,
    pub transaction_date: NaiveDate,
    pub due_date: Option<NaiveDate>,
    pub period: String,
    pub gl_entry_id: Option<i64>,
    pub description: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

impl ArApLedger {
    /// 未清金额 = amount - amount_applied
    pub fn outstanding(&self) -> Decimal {
        self.amount - self.amount_applied
    }

    /// 是否已全部核销
    pub fn is_fully_settled(&self) -> bool {
        self.outstanding() <= Decimal::ZERO
    }
}

/// 核销明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArApSettlement {
    pub id: i64,
    pub payment_source_type: DocumentType,
    pub payment_source_id: i64,
    pub invoice_source_type: DocumentType,
    pub invoice_source_id: i64,
    pub amount: Decimal,
    pub payment_ledger_id: Option<i64>,
    pub invoice_ledger_id: Option<i64>,
    pub exchange_gain_loss: Decimal,
    pub settlement_date: NaiveDate,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 创建核销请求
#[derive(Debug, Clone)]
pub struct SettleReq {
    /// 付款来源类型（通常为 CashJournal）
    pub payment_source_type: DocumentType,
    /// 付款来源 ID
    pub payment_source_id: i64,
    /// 发票来源类型（SalesInvoice / PurchaseInvoice）
    pub invoice_source_type: DocumentType,
    /// 发票来源 ID
    pub invoice_source_id: i64,
    /// 本次核销金额
    pub amount: Decimal,
}

/// 台账查询筛选条件
#[derive(Debug, Clone, Default)]
pub struct ArApLedgerFilter {
    pub party_type: Option<CounterpartyType>,
    pub party_id: Option<i64>,
    /// 仅显示未清项（amount_outstanding > 0）
    pub outstanding_only: bool,
    pub period: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

/// 核销记录查询筛选条件
#[derive(Debug, Clone, Default)]
pub struct SettlementFilter {
    pub payment_source_type: Option<DocumentType>,
    pub payment_source_id: Option<i64>,
    pub invoice_source_type: Option<DocumentType>,
    pub invoice_source_id: Option<i64>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

/// 账龄分析请求
#[derive(Debug, Clone)]
pub struct AgingReq {
    /// 往来方类型
    pub party_type: CounterpartyType,
    /// 截止日期（默认当天）
    pub as_of_date: NaiveDate,
    /// 账龄基准
    pub ageing_based_on: AgeingBasis,
    /// 账龄分段天数，如 [30, 60, 90, 120]
    pub buckets: Vec<i32>,
    /// 可选：限定往来方
    pub party_ids: Option<Vec<i64>>,
}

// ---------------------------------------------------------------------------
// Response / View types
// ---------------------------------------------------------------------------

/// 台账查询行（含 JOIN 的往来方名称和科目信息）
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ArApLedgerRow {
    pub id: i64,
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub party_name: String,
    pub account_code: String,
    pub account_name: String,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_doc_no: String,
    pub direction: LedgerDirection,
    pub amount: Decimal,
    pub amount_applied: Decimal,
    pub amount_outstanding: Decimal,
    pub currency: String,
    pub transaction_date: NaiveDate,
    pub due_date: Option<NaiveDate>,
    pub period: String,
    pub description: String,
}

/// 往来方余额
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PartyBalance {
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub party_name: String,
    /// AR 总额（所有 Debit 方向未清金额之和）
    pub total_ar: Decimal,
    /// AP 总额（所有 Credit 方向未清金额之和）
    pub total_ap: Decimal,
    /// 净额（正=应收债权，负=应付债务）
    pub net_balance: Decimal,
    pub currency: String,
}

/// 账龄分析行（按往来方一行）
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgingRow {
    pub party_id: i64,
    pub party_name: String,
    pub total_outstanding: Decimal,
    /// 各账龄桶金额，顺序与 AgingReq.buckets 对应
    pub buckets: Vec<Decimal>,
    /// 超出最大分段的金额
    pub over_max: Decimal,
}

/// 未清发票（用于核销选择器）
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct OpenInvoice {
    pub source_type: DocumentType,
    pub source_id: i64,
    pub doc_number: String,
    pub issue_date: NaiveDate,
    pub due_date: Option<NaiveDate>,
    pub total: Decimal,
    pub outstanding: Decimal,
    pub currency: String,
}

/// 未分配的收款/付款（用于核销选择器）
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct UnappliedPayment {
    pub source_type: DocumentType,
    pub source_id: i64,
    pub doc_number: String,
    pub transaction_date: NaiveDate,
    pub amount: Decimal,
    pub unapplied: Decimal,
    pub currency: String,
}

/// 核销结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SettleResult {
    pub settlement_id: i64,
    pub payment_ledger_id: i64,
    pub invoice_ledger_id: i64,
}
