use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::wms::enums::{BinStatus, WarehouseStatus, WarehouseType, ZoneType};

/// 仓库实体 — 映射 warehouses 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Warehouse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub warehouse_type: WarehouseType,
    pub status: WarehouseStatus,
    pub address: Option<String>,
    pub manager_id: Option<i64>,
    pub is_virtual: bool,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 库区实体 — 映射 zones 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Zone {
    pub id: i64,
    pub warehouse_id: i64,
    pub code: String,
    pub name: String,
    pub zone_type: ZoneType,
    pub sort_order: i32,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 库位实体 — 映射 bins 表
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Bin {
    pub id: i64,
    pub zone_id: i64,
    pub code: String,
    pub name: String,
    pub row_no: Option<String>,
    pub column_no: Option<String>,
    pub layer_no: Option<String>,
    pub capacity_limit: Option<Decimal>,
    pub allowed_product_types: Option<Vec<String>>,
    pub temperature_req: Option<String>,
    pub status: BinStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 创建仓库请求
#[derive(Debug, Clone)]
pub struct CreateWarehouseReq {
    pub code: String,
    pub name: String,
    pub warehouse_type: WarehouseType,
    pub address: Option<String>,
    pub manager_id: Option<i64>,
    pub is_virtual: bool,
    pub remark: String,
}

/// 更新仓库请求 — 所有字段可选，仅更新提供的字段
#[derive(Debug, Clone, Default)]
pub struct UpdateWarehouseReq {
    pub name: Option<String>,
    pub warehouse_type: Option<WarehouseType>,
    pub address: Option<String>,
    pub manager_id: Option<i64>,
    pub is_virtual: Option<bool>,
    pub remark: Option<String>,
    pub status: Option<WarehouseStatus>,
}

/// 仓库查询过滤
#[derive(Debug, Clone, Default)]
pub struct WarehouseFilter {
    pub warehouse_type: Option<WarehouseType>,
    pub status: Option<WarehouseStatus>,
    pub keyword: Option<String>,
}

/// 创建库区请求
#[derive(Debug, Clone)]
pub struct CreateZoneReq {
    pub code: String,
    pub name: String,
    pub zone_type: ZoneType,
    pub sort_order: Option<i32>,
    pub remark: Option<String>,
}

/// 更新库区请求 — 所有字段可选，仅更新提供的字段
#[derive(Debug, Clone, Default)]
pub struct UpdateZoneReq {
    pub name: Option<String>,
    pub zone_type: Option<ZoneType>,
    pub sort_order: Option<i32>,
    pub remark: Option<String>,
}

/// 创建库位请求
#[derive(Debug, Clone)]
pub struct CreateBinReq {
    pub code: String,
    pub name: String,
    pub row_no: Option<String>,
    pub column_no: Option<String>,
    pub layer_no: Option<String>,
    pub capacity_limit: Option<Decimal>,
    pub allowed_product_types: Option<Vec<String>>,
    pub temperature_req: Option<String>,
}

/// 更新库位请求 — 所有字段可选，仅更新提供的字段
#[derive(Debug, Clone, Default)]
pub struct UpdateBinReq {
    pub name: Option<String>,
    pub row_no: Option<String>,
    pub column_no: Option<String>,
    pub layer_no: Option<String>,
    pub capacity_limit: Option<Decimal>,
    pub allowed_product_types: Option<Vec<String>>,
    pub temperature_req: Option<String>,
    pub status: Option<BinStatus>,
}

/// 库位查询过滤
#[derive(Debug, Clone, Default)]
pub struct BinFilter {
    pub status: Option<BinStatus>,
}

/// 仓库下库位查询参数（含分页）
pub struct ListBinsByWarehouseParams {
    pub warehouse_id: i64,
    pub keyword: Option<String>,
    pub is_active: Option<bool>,
    pub page: u32,
    pub page_size: u32,
}

/// 跨仓库搜索库位参数（含分页）
pub struct SearchBinsParams {
    pub keyword: Option<String>,
    pub is_active: Option<bool>,
    pub warehouse_id: Option<i64>,
    pub page: u32,
    pub page_size: u32,
}

/// 库位 + 仓库关联信息（用于 Location 兼容查询）
#[derive(Debug, Clone)]
pub struct BinWithWarehouse {
    pub bin: Bin,
    pub warehouse_id: i64,
    pub warehouse_name: String,
}

/// 仓库库存统计
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WarehouseInventoryStats {
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub total_quantity: Decimal,
    pub bin_count: i64,
    pub product_count: i64,
    pub low_stock_count: i64,
}

/// 库位库存统计
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BinInventoryStats {
    pub bin_id: i64,
    pub bin_code: String,
    pub bin_name: String,
    pub total_quantity: Decimal,
    pub product_count: i64,
    pub low_stock_count: i64,
}
