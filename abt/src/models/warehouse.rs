//! 仓库数据模型
//!
//! 包含仓库实体及其相关结构。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 仓库实体
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Warehouse {
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub warehouse_code: String,
    pub status: WarehouseStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, PgRow> for Warehouse {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let status_str: String = row.try_get("status")?;
        let status = match status_str.as_str() {
            "active" => WarehouseStatus::Active,
            "inactive" => WarehouseStatus::Inactive,
            _ => WarehouseStatus::Active,
        };

        Ok(Warehouse {
            warehouse_id: row.try_get("warehouse_id")?,
            warehouse_name: row.try_get("warehouse_name")?,
            warehouse_code: row.try_get("warehouse_code")?,
            status,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at")?,
        })
    }
}

/// 仓库状态
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WarehouseStatus {
    #[default]
    Active,
    Inactive,
}

impl std::fmt::Display for WarehouseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarehouseStatus::Active => write!(f, "active"),
            WarehouseStatus::Inactive => write!(f, "inactive"),
        }
    }
}

/// 创建仓库请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWarehouseRequest {
    pub warehouse_name: String,
    pub warehouse_code: String,
}

/// 更新仓库请求
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateWarehouseRequest {
    pub warehouse_name: String,
    pub warehouse_code: Option<String>,
    pub status: WarehouseStatus,
}

/// 仓库带库位信息（查询用）
/// 注：locations 字段将在 Location 模块实现后可用
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WarehouseWithLocations {
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub warehouse_code: String,
    pub status: WarehouseStatus,
    #[serde(default)]
    pub locations: Vec<LocationInfo>,
}

/// 库位简要信息（用于 WarehouseWithLocations）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LocationInfo {
    pub location_id: i64,
    pub location_code: String,
    pub location_name: Option<String>,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warehouse_default() {
        let warehouse = Warehouse::default();
        assert_eq!(warehouse.warehouse_id, 0);
        assert!(warehouse.warehouse_name.is_empty());
        assert!(warehouse.warehouse_code.is_empty());
        assert_eq!(warehouse.status, WarehouseStatus::Active);
    }

    #[test]
    fn test_warehouse_status_display() {
        assert_eq!(format!("{}", WarehouseStatus::Active), "active");
        assert_eq!(format!("{}", WarehouseStatus::Inactive), "inactive");
    }

    #[test]
    fn test_warehouse_status_serialization() {
        let status = WarehouseStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""active""#);

        let deserialized: WarehouseStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, WarehouseStatus::Active);
    }

    #[test]
    fn test_create_warehouse_request() {
        let request = CreateWarehouseRequest {
            warehouse_name: "主仓库".to_string(),
            warehouse_code: "MAIN".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""warehouse_name":"主仓库""#));
        assert!(json.contains(r#""warehouse_code":"MAIN""#));

        let deserialized: CreateWarehouseRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.warehouse_name, "主仓库");
        assert_eq!(deserialized.warehouse_code, "MAIN");
    }

    #[test]
    fn test_update_warehouse_request() {
        let request = UpdateWarehouseRequest {
            warehouse_name: "更新后的仓库".to_string(),
            warehouse_code: Some("NEW_CODE".to_string()),
            status: WarehouseStatus::Inactive,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: UpdateWarehouseRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.warehouse_name, "更新后的仓库");
        assert_eq!(deserialized.warehouse_code, Some("NEW_CODE".to_string()));
        assert_eq!(deserialized.status, WarehouseStatus::Inactive);
    }
}
