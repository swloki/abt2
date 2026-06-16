use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

/// 供应商产品价格实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SupplierProductPrice {
    pub id: i64,
    pub supplier_id: i64,
    pub product_id: i64,
    pub supplier_item_code: Option<String>,
    pub supplier_item_name: Option<String>,
    pub min_order_qty: Decimal,
    pub price: Decimal,
    pub currency_code: String,
    pub discount_pct: Decimal,
    pub lead_time_days: i32,
    pub tax_rate_id: Option<i64>,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub sequence: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 价格目录列表查询参数
#[derive(Debug, Clone, Default)]
pub struct PriceListQuery {
    pub supplier_id: Option<i64>,
    pub product_id: Option<i64>,
    pub keyword: Option<String>,
    pub currency_code: Option<String>,
    pub is_active: Option<bool>,
}

/// 价格目录视图（JOIN 供应商名/产品名，用于列表与编辑回填展示）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PriceView {
    pub id: i64,
    pub supplier_id: i64,
    pub product_id: i64,
    pub supplier_item_code: Option<String>,
    pub supplier_item_name: Option<String>,
    pub min_order_qty: Decimal,
    pub price: Decimal,
    pub currency_code: String,
    pub discount_pct: Decimal,
    pub lead_time_days: i32,
    pub tax_rate_id: Option<i64>,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub sequence: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// JOIN 字段
    pub supplier_name: String,
    pub supplier_code: String,
    pub product_code: String,
    pub product_name: String,
}

/// 创建/更新价格请求（完整字段）
#[derive(Debug, Clone)]
pub struct PriceUpsertRequest {
    pub supplier_id: i64,
    pub product_id: i64,
    pub price: Decimal,
    pub currency_code: String,
    pub min_order_qty: Decimal,
    pub discount_pct: Decimal,
    pub lead_time_days: i32,
    pub tax_rate_id: Option<i64>,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub sequence: i32,
    pub supplier_item_code: Option<String>,
    pub supplier_item_name: Option<String>,
    pub is_active: bool,
}
