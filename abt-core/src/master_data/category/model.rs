use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 分类元数据 (JSONB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMeta {
    pub count: i64,
}

impl Default for CategoryMeta {
    fn default() -> Self {
        Self { count: 0 }
    }
}

/// 产品分类实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Category {
    pub category_id: i64,
    pub category_name: String,
    pub parent_id: i64,
    pub path: String,
    pub meta: serde_json::Value,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 分类树节点
#[derive(Debug, Clone)]
pub struct CategoryTree {
    pub category_id: i64,
    pub category_name: String,
    pub parent_id: i64,
    pub path: String,
    pub meta: CategoryMeta,
    pub children: Vec<CategoryTree>,
}

/// 产品-分类关联
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductCategory {
    pub product_id: i64,
    pub category_id: i64,
}

/// 创建分类请求
#[derive(Debug, Clone)]
pub struct CreateCategoryReq {
    pub category_name: String,
    pub parent_id: i64,
}

/// 更新分类请求
#[derive(Debug, Clone, Default)]
pub struct UpdateCategoryReq {
    pub category_name: Option<String>,
}

/// 分类查询过滤
#[derive(Debug, Clone, Default)]
pub struct CategoryQuery {
    pub name: Option<String>,
    pub parent_id: Option<i64>,
}
