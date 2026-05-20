//! 供应商价格数据模型
//!
//! 包含供应商报价实体及查询参数。

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 供应商价格实体
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierPrice {
    pub price_id: i64,
    pub supplier_id: i64,
    pub product_id: i64,
    pub unit_price: Decimal,
    pub valid_from: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// 供应商价格详情（含产品信息）
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct SupplierPriceDetail {
    pub price_id: i64,
    pub supplier_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub valid_from: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// 供应商价格查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SupplierPriceQuery {
    /// 供应商 ID 过滤
    pub supplier_id: Option<i64>,
    /// 产品 ID 过滤
    pub product_id: Option<i64>,
    /// 仅返回当前有效的价格
    pub active_only: Option<bool>,
    /// 页码
    pub page: Option<i64>,
    /// 每页数量
    pub page_size: Option<i64>,
}
