use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::wms::enums::LockStatus;

/// 库存锁定实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryLock {
    pub id: i64,
    pub doc_number: String,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub locked_qty: Decimal,
    pub lock_reason: String,
    pub customer_id: Option<i64>,
    pub status: LockStatus,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 创建锁定请求
#[derive(Debug, Clone)]
pub struct CreateLockReq {
    pub product_id: i64,
    pub warehouse_id: i64,
    pub locked_qty: Decimal,
    pub lock_reason: String,
    pub customer_id: Option<i64>,
}

/// 锁定查询过滤
#[derive(Debug, Clone, Default)]
pub struct LockFilter {
    pub status: Option<LockStatus>,
    pub product_id: Option<i64>,
    pub warehouse_id: Option<i64>,
    pub customer_id: Option<i64>,
}
