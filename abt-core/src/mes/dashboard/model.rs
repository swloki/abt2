use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::FromRow;

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
