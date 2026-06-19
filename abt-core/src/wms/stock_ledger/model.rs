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
    /// 反范式预留量：**仅含 InventoryLock**（质量冻结/客户质押），不含 InventoryReservation。
    /// 业务预留（订单/工单）记录在独立的 inventory_reservations 表。
    pub reserved_qty: Decimal,
    /// 反范式可用量 = quantity − reserved_qty（仅扣 Lock）。**不要把它当作对外可用量展示或决策依据**——
    /// 对外可用量（ATP）必须用 InventoryTransactionService::query_available()，
    /// 它 = quantity − Lock − InventoryReservation。
    pub available_qty: Decimal,
    pub unit_cost: Option<Decimal>,
    pub received_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    /// 安全库存（DB 已有此列；list_low_stock 用 quantity < safety_stock 判定低库存）
    #[sqlx(default)]
    pub safety_stock: Option<Decimal>,
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
    pub product_ids: Option<Vec<i64>>,
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
