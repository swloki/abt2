//! 库存数据模型
//!
//! 包含库存实体及其相关结构。

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 库存实体
#[derive(Debug, Serialize, Deserialize, Clone, Default, FromRow)]
pub struct Inventory {
    pub inventory_id: i64,
    pub product_id: i64,
    pub location_id: i64,
    pub quantity: Decimal,
    pub safety_stock: Decimal,
    pub batch_no: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 库存详情（带产品和库位信息）
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct InventoryDetail {
    pub inventory_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub location_id: i64,
    pub location_code: String,
    pub warehouse_name: String,
    pub quantity: Decimal,
    pub safety_stock: Decimal,
    pub is_low_stock: bool,
    pub batch_no: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 操作类型
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    #[default]
    In, // 入库
    Out,      // 出库
    Transfer, // 调拨
    Adjust,   // 盘点调整
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::In => write!(f, "in"),
            OperationType::Out => write!(f, "out"),
            OperationType::Transfer => write!(f, "transfer"),
            OperationType::Adjust => write!(f, "adjust"),
        }
    }
}

impl std::str::FromStr for OperationType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in" => Ok(OperationType::In),
            "out" => Ok(OperationType::Out),
            "transfer" => Ok(OperationType::Transfer),
            "adjust" => Ok(OperationType::Adjust),
            _ => Err(anyhow::anyhow!("无效的操作类型: {}", s)),
        }
    }
}

/// 库存变动请求
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StockChangeRequest {
    pub product_id: i64,
    pub location_id: i64,
    pub quantity: Decimal,
    pub operation_type: OperationType,
    pub ref_order_type: Option<String>,
    pub ref_order_id: Option<String>,
    pub operator: Option<String>,
    pub remark: Option<String>,
}

impl Default for StockChangeRequest {
    fn default() -> Self {
        Self {
            product_id: 0,
            location_id: 0,
            quantity: Decimal::ZERO,
            operation_type: OperationType::In,
            ref_order_type: None,
            ref_order_id: None,
            operator: None,
            remark: None,
        }
    }
}

/// 库存调拨请求
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StockTransferRequest {
    pub product_id: i64,
    pub from_location_id: i64,
    pub to_location_id: i64,
    pub quantity: Decimal,
    pub operator: Option<String>,
    pub remark: Option<String>,
}

/// 设置安全库存请求
#[derive(Debug, Serialize, Deserialize)]
pub struct SetSafetyStockRequest {
    pub product_id: i64,
    pub location_id: i64,
    pub safety_stock: Decimal,
}

/// 库存查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InventoryQuery {
    pub product_id: Option<i64>,
    pub product_name: Option<String>,
    pub product_code: Option<String>,
    pub warehouse_id: Option<i64>,
    pub location_id: Option<i64>,
    pub term_id: Option<i64>,
    pub low_stock_only: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 日志查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InventoryLogQuery {
    pub product_id: Option<i64>,
    pub product_name: Option<String>, // 按产品名称筛选
    pub product_code: Option<String>, // 按产品编码筛选
    pub location_id: Option<i64>,
    pub warehouse_id: Option<i64>, // 按仓库筛选
    pub operation_type: Option<OperationType>,
    pub operator: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

// ============================================================================
// 库存变动日志详情（带产品和库位信息）
// ============================================================================

/// 库存变动日志详情（带产品和库位信息）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InventoryLogDetail {
    pub log_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub location_id: i64,
    pub location_code: String,
    pub warehouse_name: String,
    pub change_qty: Decimal,
    pub before_qty: Decimal,
    pub after_qty: Decimal,
    pub operation_type: OperationType,
    pub ref_order_type: Option<String>,
    pub ref_order_id: Option<String>,
    pub operator: Option<String>,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, PgRow> for InventoryLogDetail {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let operation_type_str: String = row.try_get("operation_type")?;
        let operation_type = operation_type_str
            .parse::<OperationType>()
            .unwrap_or(OperationType::In);

        Ok(InventoryLogDetail {
            log_id: row.try_get("log_id")?,
            product_id: row.try_get("product_id")?,
            product_name: row.try_get("product_name")?,
            product_code: row.try_get("product_code")?,
            location_id: row.try_get("location_id")?,
            location_code: row.try_get("location_code")?,
            warehouse_name: row.try_get("warehouse_name")?,
            change_qty: row.try_get("change_qty")?,
            before_qty: row.try_get("before_qty")?,
            after_qty: row.try_get("after_qty")?,
            operation_type,
            ref_order_type: row.try_get("ref_order_type")?,
            ref_order_id: row.try_get("ref_order_id")?,
            operator: row.try_get("operator")?,
            remark: row.try_get("remark")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

// ============================================================================
// 库存统计
// ============================================================================

/// 仓库库存统计
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct WarehouseInventoryStats {
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub warehouse_code: String,
    pub total_quantity: Decimal,
    pub location_count: i64,
    pub product_count: i64,
    pub low_stock_count: i64,
}

/// 库位库存统计
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct LocationInventoryStats {
    pub location_id: i64,
    pub location_code: String,
    pub location_name: Option<String>,
    pub total_quantity: Decimal,
    pub product_count: i64,
    pub low_stock_count: i64,
}

// ============================================================================
// Excel 导出
// ============================================================================

/// 库存导出行（用于 Excel 导出）
#[derive(Debug, Clone, FromRow)]
pub struct InventoryExportRow {
    pub product_id: i64,
    pub pdt_name: String,
    pub product_code: String,
    pub specification: String,
    pub unit: String,
    pub warehouse_name: String,
    pub location_code: String,
    pub quantity: Decimal,
    pub safety_stock: Decimal,
    pub price: Decimal,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_default() {
        let inventory = Inventory::default();
        assert_eq!(inventory.inventory_id, 0);
        assert_eq!(inventory.product_id, 0);
        assert_eq!(inventory.location_id, 0);
        assert_eq!(inventory.quantity, Decimal::ZERO);
        assert_eq!(inventory.safety_stock, Decimal::ZERO);
        assert!(inventory.batch_no.is_none());
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(format!("{}", OperationType::In), "in");
        assert_eq!(format!("{}", OperationType::Out), "out");
        assert_eq!(format!("{}", OperationType::Transfer), "transfer");
        assert_eq!(format!("{}", OperationType::Adjust), "adjust");
    }

    #[test]
    fn test_operation_type_from_str() {
        assert_eq!("in".parse::<OperationType>().unwrap(), OperationType::In);
        assert_eq!("out".parse::<OperationType>().unwrap(), OperationType::Out);
        assert!("invalid".parse::<OperationType>().is_err());
    }

    #[test]
    fn test_operation_type_serialization() {
        let op = OperationType::In;
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(json, r#""in""#);

        let deserialized: OperationType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, OperationType::In);
    }

    #[test]
    fn test_stock_change_request_default() {
        let req = StockChangeRequest::default();
        assert_eq!(req.product_id, 0);
        assert_eq!(req.quantity, Decimal::ZERO);
        assert_eq!(req.operation_type, OperationType::In);
        assert!(req.ref_order_type.is_none());
    }

    #[test]
    fn test_stock_change_request_serialization() {
        let req = StockChangeRequest {
            product_id: 1,
            location_id: 2,
            quantity: Decimal::from(100),
            operation_type: OperationType::In,
            ref_order_type: Some("purchase_order".to_string()),
            ref_order_id: Some("PO-001".to_string()),
            operator: Some("admin".to_string()),
            remark: Some("采购入库".to_string()),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""product_id":1"#));
        assert!(json.contains(r#""quantity":"100""#));
        assert!(json.contains(r#""operation_type":"in""#));

        let deserialized: StockChangeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.product_id, 1);
        assert_eq!(deserialized.quantity, Decimal::from(100));
        assert_eq!(
            deserialized.ref_order_type,
            Some("purchase_order".to_string())
        );
    }

    #[test]
    fn test_set_safety_stock_request() {
        let req = SetSafetyStockRequest {
            product_id: 1,
            location_id: 2,
            safety_stock: Decimal::from(50),
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: SetSafetyStockRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.product_id, 1);
        assert_eq!(deserialized.location_id, 2);
        assert_eq!(deserialized.safety_stock, Decimal::from(50));
    }

    #[test]
    fn test_stock_transfer_request() {
        let req = StockTransferRequest {
            product_id: 1,
            from_location_id: 10,
            to_location_id: 20,
            quantity: Decimal::from(50),
            operator: Some("admin".to_string()),
            remark: Some("调拨测试".to_string()),
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: StockTransferRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.product_id, 1);
        assert_eq!(deserialized.from_location_id, 10);
        assert_eq!(deserialized.to_location_id, 20);
        assert_eq!(deserialized.quantity, Decimal::from(50));
    }

    #[test]
    fn test_inventory_query_default() {
        let query = InventoryQuery::default();
        assert!(query.product_id.is_none());
        assert!(query.product_name.is_none());
        assert!(query.warehouse_id.is_none());
        assert!(query.location_id.is_none());
        assert!(query.low_stock_only.is_none());
        assert!(query.page.is_none());
        assert!(query.page_size.is_none());
    }

    #[test]
    fn test_inventory_detail() {
        let detail = InventoryDetail {
            inventory_id: 1,
            product_id: 100,
            product_name: "测试产品".to_string(),
            product_code: "PROD001".to_string(),
            location_id: 10,
            location_code: "A-01".to_string(),
            warehouse_name: "主仓库".to_string(),
            quantity: Decimal::from(50),
            safety_stock: Decimal::from(100),
            is_low_stock: true,
            updated_at: Some(Utc::now()),
            batch_no: Some("BATCH001".to_string()),
        };

        assert!(detail.is_low_stock);
        assert_eq!(detail.product_name, "测试产品");
    }
}
