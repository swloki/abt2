use chrono::DateTime;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::super::enums::*;
use crate::mes::production_batch::{ProductionBatch, WorkOrderRouting};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkOrder {
    pub id: i64,
    pub doc_number: String,
    pub plan_item_id: Option<i64>,
    pub product_id: i64,
    pub bom_snapshot_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub status: WorkOrderStatus,
    pub work_center_id: Option<i64>,
    pub sales_order_id: Option<i64>,
    pub version: i32,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub completed_qty: Decimal,
    pub scrap_qty: Decimal,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    // 列表展示用聚合/关联字段：list 查询通过子查询/JOIN 填充；insert/get_by_id 为 None
    pub completed_steps: Option<i32>,
    pub total_steps: Option<i32>,
    pub source_plan_id: Option<i64>,
    pub source_plan_doc: Option<String>,
    pub source_so_doc: Option<String>,
    pub source_customer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkOrderReq {
    pub plan_item_id: Option<i64>,
    pub product_id: i64,
    pub bom_snapshot_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub work_center_id: Option<i64>,
    pub sales_order_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct WorkOrderFilter {
    pub status: Option<WorkOrderStatus>,
    pub product_id: Option<i64>,
    pub keyword: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    /// 按产品编码模糊筛选（ILIKE）
    pub product_code: Option<String>,
}

// ============================================================================
// 工单工作台聚合视图（WorkOrderHubSummary）
// ============================================================================
// `get_hub_summary` 的返回类型，聚合工作台 detail-header + 摘要带 + 6 disclosure 全部数据。
// 自包含 DTO：除 WorkOrder/WorkOrderRouting/ProductionBatch（mes 内）外不跨模块耦合，
// impl 阶段从各 service 取数转换。

/// 状态步骤条节点（detail-header statusbar）
#[derive(Debug, Clone)]
pub struct StatusStep {
    pub key: &'static str,
    pub label: &'static str,
    pub state: StepState,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    Active,
    Pending,
}

/// 来源链（detail-header source-trace）
#[derive(Debug, Clone)]
pub struct SourceChain {
    pub sales_order_doc: Option<String>,
    pub customer_name: Option<String>,
    pub plan_doc: Option<String>,
    pub batch_count: i64,
    pub received_qty: Decimal,
}

/// 物料可用性 4 级（对齐 Odoo components_availability）
#[derive(Debug, Clone)]
pub struct MaterialAvailability {
    pub level: MaterialAvailabilityLevel,
    /// 最严重缺料/迟料物料名（徽章文案，如 "PCB基板"）
    pub headline: Option<String>,
    pub lines: Vec<MaterialAvailabilityLine>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialAvailabilityLevel {
    /// 绿：ATP ≥ 需求（现货-硬预留充足）
    Available,
    /// 蓝：齐套需等在途，最晚到货日 ≤ 计划开工日
    Expected,
    /// 黄：齐套需等在途，最晚到货日 > 计划开工日
    Late,
    /// 红：ATP 不足且在途补不齐
    Unavailable,
}
#[derive(Debug, Clone)]
pub struct MaterialAvailabilityLine {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub required_qty: Decimal,
    pub issued_qty: Decimal,
    pub atp: Decimal,
    pub projected: Decimal,
    pub level: MaterialAvailabilityLevel,
}

/// ① 工单信息 disclosure
#[derive(Debug, Clone)]
pub struct HubInfo {
    pub bom_snapshot_doc: Option<String>,
    pub routing_doc: Option<String>,
    pub routing_step_count: usize,
    pub consumption_mode_label: String,
    pub team_label: Option<String>,
}

/// ② 物料 & 领料 disclosure
#[derive(Debug, Clone)]
pub struct HubMaterial {
    pub requisitions: Vec<HubRequisition>,
    pub availability: MaterialAvailability,
}
#[derive(Debug, Clone)]
pub struct HubRequisition {
    pub doc_number: String,
    pub status_label: String,
    pub item_count: i64,
    pub total_qty: Decimal,
    pub items: Vec<HubRequisitionItem>,
}
#[derive(Debug, Clone)]
pub struct HubRequisitionItem {
    pub product_code: String,
    pub product_name: String,
    pub required_qty: Decimal,
    pub issued_qty: Decimal,
    pub available_qty: Decimal,
}

/// ③ 批次 × 工序矩阵 disclosure
#[derive(Debug, Clone)]
pub struct HubRoutingMatrix {
    /// 列：工序（按 step_no 升序）
    pub routings: Vec<WorkOrderRouting>,
    /// 行：每批次
    pub rows: Vec<RoutingMatrixRow>,
}
#[derive(Debug, Clone)]
pub struct RoutingMatrixRow {
    pub batch: ProductionBatch,
    /// 与 routings 对齐的单元格
    pub cells: Vec<RoutingMatrixCell>,
}
#[derive(Debug, Clone)]
pub struct RoutingMatrixCell {
    pub step_no: i32,
    pub status: RoutingCellStatus,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingCellStatus {
    Done,
    Active,
    Pending,
}

/// ④ 报工记录 disclosure（带聚合）
#[derive(Debug, Clone)]
pub struct HubReports {
    pub items: Vec<HubReportRow>,
    pub total_count: usize,
    pub total_completed: Decimal,
    pub total_defect: Decimal,
}
#[derive(Debug, Clone)]
pub struct HubReportRow {
    pub reported_at: Option<DateTime<chrono::Utc>>,
    pub batch_no: String,
    pub op_name: String,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub worker_name: String,
    pub team_label: Option<String>,
}

/// ⑤ 入库 & 质检 disclosure
#[derive(Debug, Clone)]
pub struct HubReceipts {
    pub items: Vec<HubReceiptRow>,
    pub total_received: Decimal,
    pub fqc_passed: bool,
    pub backflush_done: bool,
}
#[derive(Debug, Clone)]
pub struct HubReceiptRow {
    pub doc_number: String,
    pub batch_no: String,
    pub received_qty: Decimal,
    pub warehouse_name: String,
    pub fqc_label: String,
    pub backflush_label: String,
}

/// ⑥ 操作日志 disclosure
#[derive(Debug, Clone)]
pub struct HubAuditLog {
    pub title: String,
    pub meta: String,
    pub is_current: bool,
}

/// 工单工作台聚合（`get_hub_summary` 返回）
#[derive(Debug, Clone)]
pub struct WorkOrderHubSummary {
    pub order: WorkOrder,
    pub product_name: String,
    pub work_center_name: Option<String>,
    pub status_steps: Vec<StatusStep>,
    pub source_chain: SourceChain,
    pub material_availability: MaterialAvailability,
    pub completion_pct: Decimal,
    pub received_qty: Decimal,
    pub in_progress_qty: Decimal,
    pub info: HubInfo,
    pub material: HubMaterial,
    pub matrix: HubRoutingMatrix,
    pub reports: HubReports,
    pub receipts: HubReceipts,
    pub audit_logs: Vec<HubAuditLog>,
}
