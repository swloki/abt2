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
    pub meta: ProductMeta,
}

impl<'r> FromRow<'r, PgRow> for Product {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let product_id: i64 = row.try_get("product_id")?;
        let pdt_name: String = row.try_get("pdt_name")?;
        let meta_value: Value = row.try_get("meta")?;
        let meta: ProductMeta =
            serde_json::from_value(meta_value).map_err(|e| sqlx::Error::ColumnDecode {
                index: "meta".to_string(),
                source: Box::new(e),
            })?;
        Ok(Product {
            product_id,
            pdt_name,
            meta,
        })
    }
}

/// 产品元数据（存储在 JSONB 字段中）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProductMeta {
    /// 产品大类
    pub category: String,
    /// 产品中类
    pub subcategory: String,
    /// 产品编码
    pub product_code: String,
    /// 规格
    pub specification: String,
    /// 单位
    pub unit: String,
    /// 获取途径
    pub acquire_channel: String,
    /// 损耗率
    pub loss_rate: f64,
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
    /// 产品元数据
    pub meta: ProductMeta,
}

/// 更新产品请求
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateProductRequest {
    /// 产品名称
    pub pdt_name: String,
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
    }

    #[test]
    fn test_product_meta_default() {
        let meta = ProductMeta::default();
        assert!(meta.category.is_empty());
        assert!(meta.subcategory.is_empty());
        assert!(meta.product_code.is_empty());
        assert!(meta.specification.is_empty());
        assert!(meta.unit.is_empty());
        assert!(meta.acquire_channel.is_empty());
        assert_eq!(meta.loss_rate, 0.0);
        assert!(meta.old_code.is_none());
    }

    #[test]
    fn test_product_serialization() {
        let product = Product {
            product_id: 1,
            pdt_name: "测试产品".to_string(),
            meta: ProductMeta {
                category: "电子".to_string(),
                subcategory: "芯片".to_string(),
                product_code: "PROD001".to_string(),
                specification: "10x10mm".to_string(),
                unit: "个".to_string(),
                acquire_channel: "采购".to_string(),
                loss_rate: 0.01,
                old_code: Some("OLD001".to_string()),
            },
        };

        let json = serde_json::to_string(&product).unwrap();
        assert!(json.contains(r#""product_id":1"#));
        assert!(json.contains(r#""pdt_name":"测试产品""#));
        assert!(json.contains(r#""product_code":"PROD001""#));

        let deserialized: Product = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.product_id, 1);
        assert_eq!(deserialized.pdt_name, "测试产品");
        assert_eq!(deserialized.meta.product_code, "PROD001");
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
            meta: ProductMeta {
                category: "分类1".to_string(),
                subcategory: "分类2".to_string(),
                product_code: "NEW001".to_string(),
                specification: "规格".to_string(),
                unit: "件".to_string(),
                acquire_channel: "自制".to_string(),
                loss_rate: 0.0,
                old_code: None,
            },
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""pdt_name":"新产品""#));

        let deserialized: CreateProductRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pdt_name, "新产品");
        assert_eq!(deserialized.meta.product_code, "NEW001");
    }
}
