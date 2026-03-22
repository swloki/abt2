//! 库位数据模型
//!
//! 包含库位实体及其相关结构。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::LocationInfo;

/// 库位实体
#[derive(Debug, Serialize, Deserialize, Clone, Default, FromRow)]
pub struct Location {
    pub location_id: i64,
    pub warehouse_id: i64,
    pub location_code: String,
    pub location_name: Option<String>,
    pub capacity: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl From<Location> for LocationInfo {
    fn from(loc: Location) -> Self {
        LocationInfo {
            location_id: loc.location_id,
            location_code: loc.location_code,
            location_name: loc.location_name,
        }
    }
}

/// 创建库位请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLocationRequest {
    pub warehouse_id: i64,
    pub location_code: String,
    pub location_name: Option<String>,
    pub capacity: Option<i32>,
}

/// 更新库位请求
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLocationRequest {
    pub location_code: String,
    pub location_name: Option<String>,
    pub capacity: Option<i32>,
}

/// 库位带仓库信息（查询用）
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct LocationWithWarehouse {
    pub location_id: i64,
    pub location_code: String,
    pub location_name: Option<String>,
    pub capacity: Option<i32>,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub warehouse_code: String,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_default() {
        let location = Location::default();
        assert_eq!(location.location_id, 0);
        assert_eq!(location.warehouse_id, 0);
        assert!(location.location_code.is_empty());
        assert!(location.location_name.is_none());
        assert!(location.capacity.is_none());
    }

    #[test]
    fn test_create_location_request() {
        let request = CreateLocationRequest {
            warehouse_id: 1,
            location_code: "A-01-02".to_string(),
            location_name: Some("货架A第1层第2格".to_string()),
            capacity: Some(100),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""warehouse_id":1"#));
        assert!(json.contains(r#""location_code":"A-01-02""#));

        let deserialized: CreateLocationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.warehouse_id, 1);
        assert_eq!(deserialized.location_code, "A-01-02");
        assert_eq!(
            deserialized.location_name,
            Some("货架A第1层第2格".to_string())
        );
        assert_eq!(deserialized.capacity, Some(100));
    }

    #[test]
    fn test_update_location_request() {
        let request = UpdateLocationRequest {
            location_code: "B-02-03".to_string(),
            location_name: Some("更新后的库位名".to_string()),
            capacity: Some(200),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: UpdateLocationRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.location_code, "B-02-03");
        assert_eq!(
            deserialized.location_name,
            Some("更新后的库位名".to_string())
        );
    }

    #[test]
    fn test_location_with_warehouse() {
        let location = LocationWithWarehouse {
            location_id: 1,
            location_code: "A-01".to_string(),
            location_name: Some("测试库位".to_string()),
            capacity: Some(50),
            warehouse_id: 10,
            warehouse_name: "主仓库".to_string(),
            warehouse_code: "MAIN".to_string(),
        };

        let json = serde_json::to_string(&location).unwrap();
        assert!(json.contains(r#""warehouse_name":"主仓库""#));

        let deserialized: LocationWithWarehouse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.warehouse_name, "主仓库");
        assert_eq!(deserialized.location_code, "A-01");
    }

    #[test]
    fn test_location_to_location_info() {
        let location = Location {
            location_id: 1,
            warehouse_id: 10,
            location_code: "A-01".to_string(),
            location_name: Some("测试库位".to_string()),
            capacity: Some(50),
            created_at: Utc::now(),
            deleted_at: None,
        };

        let info: LocationInfo = location.into();
        assert_eq!(info.location_id, 1);
        assert_eq!(info.location_code, "A-01");
        assert_eq!(info.location_name, Some("测试库位".to_string()));
    }
}
