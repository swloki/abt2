use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 分类元数据 (JSONB)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryMeta {
    pub count: i64,
}

impl sqlx::Type<sqlx::Postgres> for CategoryMeta {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <serde_json::Value as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for CategoryMeta {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let val = serde_json::to_value(self)?;
        <serde_json::Value as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&val, buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for CategoryMeta {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let val = <serde_json::Value as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Ok(serde_json::from_value(val)?)
    }
}

/// 产品分类实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Category {
    pub category_id: i64,
    pub category_name: String,
    pub parent_id: i64,
    pub path: String,
    pub meta: CategoryMeta,
    pub created_at: DateTime<Utc>,
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

/// 产品摘要（分类详情页关联产品列表用）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductSummary {
    pub product_id: i64,
    pub product_code: String,
    pub pdt_name: String,
    pub status: crate::master_data::product::ProductStatus,
    pub spec: Option<String>,
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
