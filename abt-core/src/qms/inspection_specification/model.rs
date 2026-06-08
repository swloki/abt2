use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::qms::enums::*;

// -- JSONB strong types --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckItem {
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub standard: String,
    #[serde(default)]
    pub tolerance: String,
    #[serde(default)]
    pub method: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SamplePlan {
    pub level: String,
    pub aql: rust_decimal::Decimal,
    pub mode: String,
}

// -- DB row model --

#[derive(Debug, Clone)]
pub struct InspectionSpecification {
    pub id: i64,
    pub doc_number: String,
    pub product_id: i64,
    pub inspection_type: InspectionType,
    pub check_items: Vec<CheckItem>,
    pub sample_plan: SamplePlan,
    pub status: SpecStatus,
    pub version: i32,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// -- Request types --

#[derive(Debug, Clone)]
pub struct CreateInspectionSpecificationReq {
    pub product_id: i64,
    pub inspection_type: InspectionType,
    pub check_items: Vec<CheckItem>,
    pub sample_plan: SamplePlan,
}

#[derive(Debug, Clone)]
pub struct UpdateInspectionSpecificationReq {
    pub check_items: Option<Vec<CheckItem>>,
    pub sample_plan: Option<SamplePlan>,
    pub status: Option<SpecStatus>,
    pub expected_version: i32,
}

// -- Filter types --

#[derive(Debug, Clone, Default)]
pub struct InspectionSpecFilter {
    pub product_id: Option<i64>,
    pub inspection_type: Option<InspectionType>,
    pub status: Option<SpecStatus>,
    pub keyword: Option<String>,
}
