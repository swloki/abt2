use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::FromRow;

use crate::mes::enums::BatchStatus;

/// Dashboard 5个统计卡片
#[derive(Debug, Clone, FromRow)]
pub struct DashboardStats {
    pub plan_count: i64,
    pub active_order_count: i64,
    pub active_batch_count: i64,
    pub pending_receipt_count: i64,
    pub completed_qty: Decimal,
}

/// 快捷入口统计
#[derive(Debug, Clone)]
pub struct QuickEntryStats {
    pub plan_total: i64,
    pub order_active: i64,
    pub batch_active: i64,
    pub report_month: i64,
    pub insp_pending: i64,
    pub receipt_pending: i64,
    pub batch_total: i64,
    pub insp_total: i64,
}

/// 最近操作记录
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RecentOp {
    pub created_at: DateTime<Utc>,
    pub op_type: String,
    pub doc_number: String,
    pub product_name: Option<String>,
    pub operator_name: Option<String>,
}

/// 排程看板统计
#[derive(Debug, Clone, FromRow)]
pub struct ScheduleStats {
    pub active_orders: i64,
    pub pending_batches: i64,
    pub in_progress_batches: i64,
    pub pending_receipt_batches: i64,
    pub completed_batches: i64,
}

/// 看板卡片
#[derive(Debug, Clone, FromRow)]
pub struct ScheduleCard {
    pub id: i64,
    pub batch_no: String,
    pub card_sn: String,
    pub product_name: Option<String>,
    pub batch_qty: Decimal,
    pub completed_qty: Decimal,
    pub current_step: i32,
    pub total_steps: Option<i32>,
    pub current_step_name: Option<String>,
    pub status: BatchStatus,
    pub work_order_id: i64,
    pub wo_doc_number: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Material Usage ──

/// 工单基础信息（用于物料消耗追踪页头部）
#[derive(Debug, Clone, FromRow)]
pub struct WoBasicInfo {
    pub id: i64,
    pub doc_number: String,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub planned_qty: Decimal,
    pub completed_qty: Decimal,
    pub status: i16,
    pub bom_snapshot_id: Option<i64>,
    pub bom_version: Option<String>,
}

/// BOM vs 实际对比行
#[derive(Debug, Clone, FromRow)]
pub struct BomCompareItem {
    pub component_id: i64,
    pub component_code: Option<String>,
    pub component_name: Option<String>,
    pub unit: Option<String>,
    pub per_unit_qty: Decimal,
    pub standard_total: Decimal,
    pub backflush_total: Decimal,
    pub picked_qty: Decimal,
}

/// 物料消耗汇总
#[derive(Debug, Clone)]
pub struct MaterialUsageSummary {
    pub standard_qty: Decimal,
    pub backflush_qty: Decimal,
    pub variance_qty: Decimal,
}

/// 数据质量统计（首页数据质量卡片）
#[derive(Debug, Clone, Default, Serialize, FromRow)]
pub struct DataQualityStats {
    pub no_routing_count: i64,
    pub no_bom_count: i64,
    pub complete_count: i64,
}
