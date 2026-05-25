use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::qms::enums::*;

#[derive(Debug, Clone)]
pub struct Mrb {
    pub id: i64,
    pub doc_number: String,
    pub inspection_result_id: i64,
    pub product_id: i64,
    pub defect_description: String,
    pub disposition: MRBDisposition,
    pub responsible_party: ResponsibleParty,
    pub cost_impact: Decimal,
    pub status: MRBStatus,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateMrbReq {
    pub inspection_result_id: i64,
    pub product_id: i64,
    pub defect_description: String,
    pub disposition: MRBDisposition,
    pub responsible_party: ResponsibleParty,
    pub cost_impact: Decimal,
    pub remark: String,
}

#[derive(Debug, Clone)]
pub struct ExecuteDispositionReq {
    pub disposition: MRBDisposition,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MrbFilter {
    pub inspection_result_id: Option<i64>,
    pub product_id: Option<i64>,
    pub disposition: Option<MRBDisposition>,
    pub status: Option<MRBStatus>,
    pub responsible_party: Option<ResponsibleParty>,
}
