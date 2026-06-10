use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::wms::enums::TransferStatus;

/// 调拨单实体 — 映射 inventory_transfers 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryTransfer {
    pub id: i64,
    pub doc_number: String,
    pub from_warehouse_id: i64,
    pub from_zone_id: Option<i64>,
    pub from_bin_id: Option<i64>,
    pub to_warehouse_id: i64,
    pub to_zone_id: Option<i64>,
    pub to_bin_id: Option<i64>,
    pub transfer_date: NaiveDate,
    pub status: TransferStatus,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    /// 列表查询时通过子查询填充的物料项数
    #[sqlx(default)]
    pub item_count: Option<i64>,
}

/// 调拨单明细实体 — 映射 transfer_items 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TransferItem {
    pub id: i64,
    pub transfer_id: i64,
    pub product_id: i64,
    pub quantity: Decimal,
    pub batch_no: Option<String>,
}

/// 创建调拨单请求
#[derive(Debug, Clone)]
pub struct CreateTransferReq {
    pub from_warehouse_id: i64,
    pub from_zone_id: Option<i64>,
    pub from_bin_id: Option<i64>,
    pub to_warehouse_id: i64,
    pub to_zone_id: Option<i64>,
    pub to_bin_id: Option<i64>,
    pub transfer_date: NaiveDate,
    pub items: Vec<CreateTransferItemReq>,
}

/// 创建调拨单明细请求
#[derive(Debug, Clone)]
pub struct CreateTransferItemReq {
    pub product_id: i64,
    pub quantity: Decimal,
    pub batch_no: Option<String>,
}

/// 调拨单查询过滤
#[derive(Debug, Clone, Default)]
pub struct TransferFilter {
    pub doc_number: Option<String>,
    pub status: Option<TransferStatus>,
    pub from_warehouse_id: Option<i64>,
    pub to_warehouse_id: Option<i64>,
}
