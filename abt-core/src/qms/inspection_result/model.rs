use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::qms::enums::*;

// -- JSONB strong types --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub item: String,
    pub measured: String,
    pub pass: bool,
    pub remark: Option<String>,
}

// -- DB row model --

#[derive(Debug, Clone)]
pub struct InspectionResult {
    pub id: i64,
    pub doc_number: String,
    pub spec_id: i64,
    pub source_type: InspectionSourceType,
    pub source_id: i64,
    pub inspection_type: InspectionType,
    pub batch_no: String,
    pub sample_qty: Decimal,
    pub qualified_qty: Decimal,
    pub unqualified_qty: Decimal,
    pub result: InspectionResultType,
    pub check_results: Vec<CheckResult>,
    pub inspector_id: i64,
    pub inspection_date: Option<NaiveDate>,
    pub status: InspectionStatus,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// -- Request types --

/// 创建检验结果 — 仅录入来源信息和样本数量
#[derive(Debug, Clone)]
pub struct CreateInspectionResultReq {
    pub spec_id: i64,
    pub source_type: InspectionSourceType,
    pub source_id: i64,
    pub batch_no: String,
    pub sample_qty: Decimal,
}

/// 记录检验结果 — 录入实际检验数据，返回 QualityGateStatus
#[derive(Debug, Clone)]
pub struct RecordInspectionResultReq {
    pub result: InspectionResultType,
    pub qualified_qty: Decimal,
    pub unqualified_qty: Decimal,
    pub check_results: Vec<CheckResult>,
    pub inspector_id: i64,
    pub inspection_date: NaiveDate,
}

// -- Filter types --

#[derive(Debug, Clone, Default)]
pub struct InspectionResultFilter {
    pub source_type: Option<InspectionSourceType>,
    pub source_id: Option<i64>,
    pub inspection_type: Option<InspectionType>,
    pub result: Option<InspectionResultType>,
    pub status: Option<InspectionStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}
