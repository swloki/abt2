use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::shared::enums::{DocumentType, ReservationStatus, ReservationType};

/// 库存预留实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryReservation {
    pub id: i64,
    pub product_id: i64,
    /// 仓库 ID；None 表示跨仓库预留（按 product 维度 ATP 汇总）
    pub warehouse_id: Option<i64>,
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
    /// 仓库 ID；None 表示跨仓库预留（按 product 维度 ATP 汇总）
    pub warehouse_id: Option<i64>,
    pub reserved_qty: Decimal,
    pub reservation_type: ReservationType,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_line_id: Option<i64>,
    pub priority: i32,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 预留明细（按产品查询，JOIN 来源单据与客户，供前端「被占用」明细展示）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReservationDetail {
    pub id: i64,
    pub reserved_qty: Decimal,
    pub reservation_type: ReservationType,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_line_id: Option<i64>,
    pub status: ReservationStatus,
    pub created_at: DateTime<Utc>,
    /// 来源单据单号（当前仅 JOIN 销售订单 source_type=2；非销售订单来源为 None）
    pub source_doc_number: Option<String>,
    /// 来源单据状态（i16，展示层按 source_type 映射文案）
    pub source_status: Option<i16>,
    /// 客户名称（来源为销售订单时）
    pub customer_name: Option<String>,
}
