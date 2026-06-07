use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::super::enums::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkReport {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub batch_id: i64,
    pub routing_id: i64,
    pub report_date: NaiveDate,
    pub shift: ShiftType,
    pub worker_id: i64,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub defect_reason: Option<DefectReason>,
    pub work_hours: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct DateRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone)]
pub struct WageSummary {
    pub worker_id: i64,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_amount: Decimal,
    pub details: Vec<WageDetail>,
}

#[derive(Debug, Clone)]
pub struct WageDetail {
    pub work_order_id: i64,
    pub batch_id: i64,
    pub routing_id: i64,
    pub process_name: String,
    pub report_date: NaiveDate,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub defect_reason: Option<DefectReason>,
    pub unit_price: Decimal,
    pub wage_amount: Decimal,
}

#[derive(Debug, Clone, Default)]
pub struct ReportDetailLookups {
    pub wo_doc_number: Option<String>,
    pub batch_no: Option<String>,
    pub process_name: Option<String>,
    pub worker_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReportListFilter {
    pub keyword: Option<String>,
    pub work_order_id: Option<i64>,
    pub shift: Option<i32>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportListItem {
    pub id: i64,
    pub doc_number: String,
    pub work_order_id: i64,
    pub batch_id: i64,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub process_name: String,
    pub step_order: i32,
    pub report_date: NaiveDate,
    pub shift: ShiftType,
    pub worker_id: i64,
    pub worker_name: Option<String>,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub work_hours: Decimal,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
