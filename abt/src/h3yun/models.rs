//! H3Yun 同步数据类型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use std::time::Duration;

/// 同步实体类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum EntityType {
    Product,
    Inventory,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Product => "product",
            EntityType::Inventory => "inventory",
        }
    }
}

/// 同步事件优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Normal,
    Low,
}

/// 同步事件 — 通过 channel 传递
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub priority: Priority,
}

/// 映射表行 — h3yun_sync_state
#[derive(Debug, Clone)]
pub struct SyncState {
    pub id: i32,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub h3yun_object_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, PgRow> for SyncState {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let entity_type_str: String = row.try_get("entity_type")?;
        let entity_type = match entity_type_str.as_str() {
            "product" => EntityType::Product,
            "inventory" => EntityType::Inventory,
            other => {
                return Err(sqlx::Error::ColumnDecode {
                    index: "entity_type".to_string(),
                    source: format!("unknown entity_type: {other}").into(),
                })
            }
        };
        Ok(SyncState {
            id: row.try_get("id")?,
            entity_type,
            entity_id: row.try_get("entity_id")?,
            h3yun_object_id: row.try_get("h3yun_object_id")?,
            last_synced_at: row.try_get("last_synced_at")?,
            content_hash: row.try_get("content_hash")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// 同步错误分类
#[derive(Debug)]
pub enum SyncError {
    /// 网络超时、429 rate limit — 可重试
    Transient { backoff_hint: Duration },
    /// 字段格式错误、必填字段缺失 — 跳过该记录
    ValidationError {
        record_id: String,
        fields: Vec<String>,
    },
    /// 认证失败、schema 不匹配 — 中止批次
    FatalError { reason: String },
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Transient { backoff_hint } => {
                write!(f, "Transient error (retry after {:?})", backoff_hint)
            }
            SyncError::ValidationError { record_id, fields } => {
                write!(
                    f,
                    "Validation error for record {}: missing/invalid fields: {}",
                    record_id,
                    fields.join(", ")
                )
            }
            SyncError::FatalError { reason } => {
                write!(f, "Fatal error: {}", reason)
            }
        }
    }
}

impl std::error::Error for SyncError {}

/// H3Yun API 请求体
#[derive(Debug, Serialize)]
pub struct H3YunRequest {
    pub ActionName: String,
    pub SchemaCode: String,
    pub BizObject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub IsSubmit: Option<bool>,
}

/// H3Yun API 响应体（通用）
#[derive(Debug, Deserialize)]
pub struct H3YunResponse {
    #[serde(default)]
    pub Successful: bool,
    #[serde(default)]
    pub ErrorMessage: String,
    #[serde(default)]
    pub ReturnData: Option<serde_json::Value>,
    #[serde(default)]
    pub ErrorCode: Option<i32>,
}

/// H3Yun CreateBizObject 响应中的 ObjectIds
#[derive(Debug, Deserialize)]
pub struct CreateResult {
    #[serde(default)]
    pub ObjectIds: Vec<String>,
}

/// H3Yun schema code 常量
pub mod schema {
    pub const PRODUCT: &str = "D000119Product_sale";
    pub const WAREHOUSE: &str = "D000119warehouse";
}

/// H3Yun action name 常量
pub mod action {
    pub const LOAD: &str = "LoadBizObjects";
    pub const CREATE: &str = "CreateBizObject";
    pub const UPDATE: &str = "UpdateBizObject";
    pub const REMOVE: &str = "RemoveBizObject";
}
