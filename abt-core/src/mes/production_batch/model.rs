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
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
    pub product_id: Option<i64>,
}

/// 批次工序执行进度（写真相源）
///
/// 每个 (batch_id, routing_id) 组合一条记录。
/// 报工事务 confirm_routing_step 的累加目标。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BatchRoutingProgress {
    pub id: i64,
    pub batch_id: i64,
    pub routing_id: i64,
    pub status: RoutingStatus,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
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
    pub wage_amount: Decimal,
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

#[derive(Debug, Clone, Default)]
pub struct BatchListFilter {
    pub status: Option<BatchStatus>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BatchListItem {
    pub id: i64,
    pub batch_no: String,
    pub card_sn: String,
    pub work_order_id: i64,
    pub wo_doc_number: Option<String>,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub batch_qty: Decimal,
    pub completed_qty: Decimal,
    pub scrap_qty: Decimal,
    pub current_step: i32,
    pub current_step_name: Option<String>,
    pub total_steps: Option<i32>,
    pub status: BatchStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
