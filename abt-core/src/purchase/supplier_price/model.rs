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
