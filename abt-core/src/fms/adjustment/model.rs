use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::enums::AdjustmentDirection;
use crate::fms::enums::CounterpartyType;

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// 应收应付调整单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArApAdjustment {
    pub id: i64,
    pub doc_number: String,
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub direction: AdjustmentDirection,
    pub amount: Decimal,
    pub currency: String,
    pub exchange_rate: Decimal,
    pub adjustment_date: NaiveDate,
    pub period: String,
    /// 内部订单号，可选（参考记录）
    pub int_order_no: Option<String>,
    /// 客户/供应商订单号，可选
    pub ext_order_no: Option<String>,
    pub description: String,
    /// 过账生成的 ar_ap_ledger.id
    pub ledger_id: Option<i64>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Input
// ---------------------------------------------------------------------------

/// 创建调整单请求（创建即过账）
#[derive(Debug, Clone)]
pub struct CreateAdjustmentReq {
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub direction: AdjustmentDirection,
    pub amount: Decimal,
    pub adjustment_date: NaiveDate,
    pub period: String,
    /// 内部订单号，可选
    pub int_order_no: Option<String>,
    /// 客户/供应商订单号，可选
    pub ext_order_no: Option<String>,
    /// 简要说明
    pub description: String,
    /// 币种（默认 CNY）— issue #69
    pub currency: String,
    /// 汇率（CNY 固定 1）— issue #69
    pub exchange_rate: Decimal,
}

// ---------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AdjustmentFilter {
    pub party_type: Option<CounterpartyType>,
    pub party_id: Option<i64>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    /// 往来方名称模糊搜
    pub keyword: Option<String>,
}

// ---------------------------------------------------------------------------
// Response / View
// ---------------------------------------------------------------------------

/// 调整单列表行（含往来方名称）
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct AdjustmentRow {
    pub id: i64,
    pub doc_number: String,
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub party_name: String,
    pub direction: AdjustmentDirection,
    pub amount: Decimal,
    pub currency: String,
    pub adjustment_date: NaiveDate,
    pub period: String,
    pub int_order_no: Option<String>,
    pub ext_order_no: Option<String>,
    pub description: String,
    pub ledger_id: Option<i64>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}
