//! 产品数据模型
//!
//! 包含产品实体及其元数据结构。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 产品实体
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Product {
    pub product_id: i64,
    pub pdt_name: String,
    pub product_code: String,
    pub unit: String,
    pub meta: ProductMeta,
}

impl<'r> FromRow<'r, PgRow> for Product {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let product_id: i64 = row.try_get("product_id")?;
        let pdt_name: String = row.try_get("pdt_name")?;
        let product_code: String = row.try_get("product_code")?;
        let unit: String = row.try_get("unit")?;
        let meta_value: Value = row.try_get("meta")?;
        let meta: ProductMeta =
            serde_json::from_value(meta_value).map_err(|e| sqlx::Error::ColumnDecode {
                index: "meta".to_string(),
                source: Box::new(e),
            })?;
        Ok(Product {
            product_id,
            pdt_name,
            product_code,
            unit,
            meta,
        })
    }
}

/// 产品元数据（存储在 JSONB 字段中，仅含低频无约束字段）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProductMeta {
    /// 规格
    pub specification: String,
    /// 获取途径
    pub acquire_channel: String,
    /// 旧编码
    pub old_code: Option<String>,
}

// ============================================================================
// 查询参数
// ============================================================================

/// 产品查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProductQuery {
    /// 产品名称（模糊匹配）
    pub pdt_name: Option<String>,
    /// 分类 ID
    pub term_id: Option<i64>,
    /// 产品编码
    pub product_code: Option<String>,
    /// 页码
    pub page: Option<i64>,
    /// 每页数量
    pub page_size: Option<i64>,
}

// ============================================================================
// 创建/更新请求
// ============================================================================

/// 创建产品请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProductRequest {
    /// 产品名称
    pub pdt_name: String,
    /// 产品编码
    pub product_code: String,
    /// 单位
    pub unit: String,
    /// 产品元数据
    pub meta: ProductMeta,
}

/// 更新产品请求
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateProductRequest {
    /// 产品名称
    pub pdt_name: String,
    /// 产品编码
    pub product_code: String,
    /// 单位
    pub unit: String,
    /// 产品元数据
    pub meta: ProductMeta,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_default() {
        let product = Product::default();
        assert_eq!(product.product_id, 0);
        assert!(product.pdt_name.is_empty());
        assert!(product.product_code.is_empty());
        assert!(product.unit.is_empty());
    }

    #[test]
    fn test_product_meta_default() {
        let meta = ProductMeta::default();
        assert!(meta.specification.is_empty());
        assert!(meta.acquire_channel.is_empty());
        assert!(meta.old_code.is_none());
    }

    #[test]
    fn test_product_meta_serialization() {
        let meta = ProductMeta {
            specification: "10x10mm".to_string(),
            acquire_channel: "采购".to_string(),
            old_code: Some("OLD001".to_string()),
        };

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains(r#""specification":"10x10mm""#));
        assert!(json.contains(r#""acquire_channel":"采购""#));
        assert!(json.contains(r#""old_code":"OLD001""#));
        // Should NOT contain removed fields
        assert!(!json.contains("category"));
        assert!(!json.contains("subcategory"));
        assert!(!json.contains("loss_rate"));
        assert!(!json.contains("product_code"));
        assert!(!json.contains("unit"));

        let deserialized: ProductMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.specification, "10x10mm");
        assert_eq!(deserialized.old_code, Some("OLD001".to_string()));
    }

    #[test]
    fn test_product_meta_old_code_none() {
        let meta = ProductMeta {
            specification: "spec".to_string(),
            acquire_channel: "channel".to_string(),
            old_code: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains(r#""old_code":null"#));

        let deserialized: ProductMeta = serde_json::from_str(&json).unwrap();
        assert!(deserialized.old_code.is_none());
    }

    #[test]
    fn test_product_query_default() {
        let query = ProductQuery::default();
        assert!(query.pdt_name.is_none());
        assert!(query.term_id.is_none());
        assert!(query.product_code.is_none());
        assert!(query.page.is_none());
        assert!(query.page_size.is_none());
    }

    #[test]
    fn test_create_product_request() {
        let request = CreateProductRequest {
            pdt_name: "新产品".to_string(),
            product_code: "NEW001".to_string(),
            unit: "件".to_string(),
            meta: ProductMeta {
                specification: "规格".to_string(),
                acquire_channel: "自制".to_string(),
                old_code: None,
            },
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""pdt_name":"新产品""#));
        assert!(json.contains(r#""product_code":"NEW001""#));
        assert!(json.contains(r#""unit":"件""#));
    }
}
