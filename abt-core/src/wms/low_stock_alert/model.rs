use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::wms::enums::LowStockAlertStatus;

/// 安全库存预警实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LowStockAlert {
    pub id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    /// 触发时的当前库存（SUM(quantity)）
    pub current_qty: Decimal,
    pub safety_stock: Decimal,
    pub status: LowStockAlertStatus,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub acked_at: Option<DateTime<Utc>>,
}

/// 预警查询过滤
#[derive(Debug, Clone, Default)]
pub struct LowStockAlertFilter {
    pub status: Option<LowStockAlertStatus>,
    pub warehouse_id: Option<i64>,
}
