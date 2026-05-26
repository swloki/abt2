use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 关注产品含库存信息
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct WatchedProductWithInventory {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub current_quantity: Decimal,
    pub effective_safety_stock: Decimal,
    pub is_alerting: bool,
}

/// 低库存关注产品（Worker 用）
#[derive(Debug, FromRow)]
pub struct LowStockWatchedProduct {
    pub product_id: i64,
    pub product_name: String,
    pub current_quantity: Decimal,
    pub effective_safety_stock: Decimal,
}

/// 产品关注者（Worker 用）
#[derive(Debug, FromRow)]
pub struct ProductWatcherUser {
    pub user_id: i64,
}
