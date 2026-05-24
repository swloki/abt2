use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::shared::enums::{DocumentType, ReservationStatus, ReservationType};

/// 库存预留实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryReservation {
    pub id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub reserved_qty: Decimal,
    pub reservation_type: ReservationType,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_line_id: Option<i64>,
    pub status: ReservationStatus,
    pub priority: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 预留请求
#[derive(Debug, Clone)]
pub struct ReserveRequest {
    pub product_id: i64,
    pub warehouse_id: i64,
    pub reserved_qty: Decimal,
    pub reservation_type: ReservationType,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_line_id: Option<i64>,
    pub priority: i32,
    pub expires_at: Option<DateTime<Utc>>,
}
