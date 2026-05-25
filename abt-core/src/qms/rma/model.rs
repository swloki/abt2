use chrono::{DateTime, NaiveDate, Utc};

use crate::qms::enums::*;

#[derive(Debug, Clone)]
pub struct Rma {
    pub id: i64,
    pub doc_number: String,
    pub customer_id: i64,
    pub sales_order_id: Option<i64>,
    pub shipping_request_id: Option<i64>,
    pub product_id: i64,
    pub linked_inspection_result_id: Option<i64>,
    pub defect_description: String,
    pub severity: Severity,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub status: RMAStatus,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateRmaReq {
    pub customer_id: i64,
    pub sales_order_id: Option<i64>,
    pub shipping_request_id: Option<i64>,
    pub product_id: i64,
    pub linked_inspection_result_id: Option<i64>,
    pub defect_description: String,
    pub severity: Severity,
    pub remark: String,
}

/// 记录根因 — 自动触发 Investigating → ActionTaken
#[derive(Debug, Clone)]
pub struct RecordRootCauseReq {
    pub root_cause: String,
    pub corrective_action: String,
}

#[derive(Debug, Clone, Default)]
pub struct RmaFilter {
    pub customer_id: Option<i64>,
    pub product_id: Option<i64>,
    pub severity: Option<Severity>,
    pub status: Option<RMAStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}
