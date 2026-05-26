use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

/// 库存台账实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StockLedger {
    pub id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub zone_id: i64,
    pub bin_id: i64,
    pub batch_no: Option<String>,
    pub quantity: Decimal,
    pub reserved_qty: Decimal,
    pub available_qty: Decimal,
    pub unit_cost: Option<Decimal>,
    pub received_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub updated_at: DateTime<Utc>,
}

/// 库存台账新增/更新请求
#[derive(Debug, Clone)]
pub struct UpsertStockReq {
    pub product_id: i64,
    pub warehouse_id: i64,
    pub zone_id: i64,
    pub bin_id: i64,
    pub batch_no: Option<String>,
    pub qty_delta: Decimal,
    pub unit_cost: Option<Decimal>,
}

/// 库存查询过滤
#[derive(Debug, Clone, Default)]
pub struct StockFilter {
    pub product_id: Option<i64>,
    pub warehouse_id: Option<i64>,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub batch_no: Option<String>,
}

/// 库存导出行（用于 Excel 导出，关联产品/仓库/库区/储位/价格/分类）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StockExportRow {
    pub product_id: i64,
    pub pdt_name: String,
    pub product_code: String,
    pub specification: Option<String>,
    pub unit: Option<String>,
    pub warehouse_name: Option<String>,
    pub zone_code: Option<String>,
    pub bin_code: Option<String>,
    pub quantity: Option<Decimal>,
    pub safety_stock: Option<Decimal>,
    pub price: Option<Decimal>,
    pub category_ids: Option<String>,
    pub category_names: Option<String>,
}

/// 没有价格记录的产品（用于 Excel 导入校验提示）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductWithoutPriceRow {
    pub product_id: i64,
    pub pdt_name: String,
    pub product_code: String,
    pub unit: Option<String>,
    pub specification: Option<String>,
}
