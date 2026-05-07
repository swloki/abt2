//! 通知数据模型

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 通知实体
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub notification_id: i64,
    pub user_id: i64,
    pub notification_type: String,
    pub title: String,
    pub content: Option<String>,
    pub related_type: Option<String>,
    pub related_id: Option<i64>,
    pub is_read: bool,
    pub read_at: Option<String>,
    pub created_at: String,
    pub metadata: Option<serde_json::Value>,
}

impl<'r> FromRow<'r, PgRow> for Notification {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let read_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("read_at")?;
        let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;

        Ok(Notification {
            notification_id: row.try_get("notification_id")?,
            user_id: row.try_get("user_id")?,
            notification_type: row.try_get("type")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            related_type: row.try_get("related_type")?,
            related_id: row.try_get("related_id")?,
            is_read: row.try_get("is_read")?,
            read_at: read_at.map(|t| t.to_rfc3339()),
            created_at: created_at.to_rfc3339(),
            metadata: row.try_get("metadata")?,
        })
    }
}

/// 通知查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NotificationQuery {
    pub notification_type: Option<String>,
    pub is_read: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

/// 创建通知请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateNotificationRequest {
    pub user_id: i64,
    pub notification_type: String,
    pub title: String,
    pub content: Option<String>,
    pub related_type: Option<String>,
    pub related_id: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

/// 关注产品含库存信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WatchedProductWithInventory {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub current_quantity: Decimal,
    pub effective_safety_stock: Decimal,
    pub is_alerting: bool,
}

impl<'r> FromRow<'r, PgRow> for WatchedProductWithInventory {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(WatchedProductWithInventory {
            product_id: row.try_get("product_id")?,
            product_code: row.try_get("product_code")?,
            product_name: row.try_get("product_name")?,
            current_quantity: row.try_get("current_quantity")?,
            effective_safety_stock: row.try_get("effective_safety_stock")?,
            is_alerting: row.try_get("is_alerting")?,
        })
    }
}

/// 未读计数（按类型分组）
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct UnreadCountByType {
    pub notification_type: String,
    pub count: i64,
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
