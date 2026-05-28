use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::super::enums::*;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductionBatch {
    pub id: i64,
    pub batch_no: String,
    pub card_sn: String,
    pub work_order_id: i64,
    pub product_id: i64,
    pub batch_qty: Decimal,
    pub completed_qty: Decimal,
    pub scrap_qty: Decimal,
    pub team_id: Option<i64>,
    pub current_step: i32,
    pub actual_start: Option<chrono::DateTime<chrono::Utc>>,
    pub actual_end: Option<chrono::DateTime<chrono::Utc>>,
    pub status: BatchStatus,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkOrderRouting {
    pub id: i64,
    pub work_order_id: i64,
    pub step_no: i32,
    pub process_name: String,
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub allowed_loss_rate: Option<Decimal>,
    pub planned_qty: Decimal,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub status: RoutingStatus,
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
}

#[derive(Debug, Clone)]
pub struct CreateBatchReq {
    pub work_order_id: i64,
    pub product_id: i64,
    pub batch_qty: Decimal,
    pub team_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SplitReq {
    pub batch_qty: Decimal,
    pub team_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct StepConfirmationReq {
    pub step_no: i32,
    pub worker_id: i64,
    pub shift: ShiftType,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub defect_reason: Option<DefectReason>,
    pub work_hours: Decimal,
    pub report_date: NaiveDate,
    pub remark: Option<String>,
}

/// 报工记录插入参数
pub struct InsertWorkReportParams<'a> {
    pub doc_number: &'a str,
    pub work_order_id: i64,
    pub batch_id: i64,
    pub routing_id: i64,
    pub report_date: chrono::NaiveDate,
    pub shift: ShiftType,
    pub worker_id: i64,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub defect_reason: Option<DefectReason>,
    pub work_hours: Decimal,
    pub remark: &'a str,
    pub operator_id: i64,
}

#[derive(Debug, Clone)]
pub struct StepConfirmationResult {
    pub work_report_id: i64,
    pub batch_id: i64,
    pub step_no: i32,
    pub next_step_no: Option<i32>,
    pub batch_status: BatchStatus,
    pub inspection_triggered: bool,
    pub wage_amount: Decimal,
}
