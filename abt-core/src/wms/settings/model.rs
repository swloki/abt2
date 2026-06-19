use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// WMS 全局设置（单行，id=1）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WmsSettings {
    pub id: i64,
    /// 盘点差异金额阈值：variance_amount 超过此值则进入 PendingReview 审批
    pub cycle_count_variance_threshold: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 更新 WMS 设置请求
#[derive(Debug, Clone)]
pub struct UpdateWmsSettingsReq {
    pub cycle_count_variance_threshold: Decimal,
}
