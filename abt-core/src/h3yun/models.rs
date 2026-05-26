//! H3Yun 同步数据类型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SyncState {
    pub id: i32,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub h3yun_object_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub enum SyncError {
    Transient { backoff_hint: Duration },
    ValidationError {
        record_id: String,
        fields: Vec<String>,
    },
    FatalError { reason: String },
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Transient { backoff_hint } => {
                write!(f, "暂时性错误，建议 {:?} 后重试", backoff_hint)
            }
            SyncError::ValidationError { record_id, fields } => {
                if record_id.is_empty() {
                    write!(f, "数据校验失败: {}", fields.join(", "))
                } else {
                    write!(f, "数据校验失败 [{}]: {}", record_id, fields.join(", "))
                }
            }
            SyncError::FatalError { reason } => {
                write!(f, "Fatal error: {}", reason)
            }
        }
    }
}

impl std::error::Error for SyncError {}

/// LoadBizObjects 用的 Filter 格式
#[derive(Debug, Serialize)]
pub struct H3YunFilter {
    pub ActionName: String,
    pub SchemaCode: String,
    pub Filter: String,
}

/// Create/Update/Remove 用的请求格式
#[derive(Debug, Serialize)]
pub struct H3YunRequest {
    pub ActionName: String,
    pub SchemaCode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub BizObject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub BizObjectId: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub IsSubmit: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct H3YunResponse {
    #[serde(default)]
    pub Successful: bool,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub ErrorMessage: String,
    #[serde(default)]
    pub ReturnData: Option<serde_json::Value>,
    #[serde(default)]
    pub ErrorCode: Option<i32>,
}

fn deserialize_null_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

pub mod schema {
    pub const PRODUCT: &str = "D000119Product_sale";
    pub const WAREHOUSE: &str = "D000119warehouse";
}

pub mod action {
    pub const LOAD: &str = "LoadBizObjects";
    pub const CREATE: &str = "CreateBizObject";
    pub const UPDATE: &str = "UpdateBizObject";
    pub const REMOVE: &str = "RemoveBizObject";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_type_as_str() {
        assert_eq!(EntityType::Product.as_str(), "product");
        assert_eq!(EntityType::Inventory.as_str(), "inventory");
    }

    #[test]
    fn sync_error_display_transient() {
        let err = SyncError::Transient { backoff_hint: Duration::from_secs(5) };
        let msg = format!("{err}");
        assert!(msg.contains("5"));
    }

    #[test]
    fn sync_error_display_validation() {
        let err = SyncError::ValidationError {
            record_id: "P001".to_string(),
            fields: vec!["bad field".to_string()],
        };
        let msg = format!("{err}");
        assert!(msg.contains("P001"));
        assert!(msg.contains("bad field"));
    }

    #[test]
    fn sync_error_display_fatal() {
        let err = SyncError::FatalError { reason: "boom".to_string() };
        let msg = format!("{err}");
        assert!(msg.contains("boom"));
    }
}
