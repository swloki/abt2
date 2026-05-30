use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 产品状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ProductStatus {
    Active = 1,
    Inactive = 2,
    Obsolete = 3,
}

impl ProductStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Active),
            2 => Some(Self::Inactive),
            3 => Some(Self::Obsolete),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for ProductStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ProductStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ProductStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ProductStatus: {v}").into())
    }
}

impl serde::Serialize for ProductStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for ProductStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ProductStatus: {v}")))
    }
}

/// 产品元数据 (JSONB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductMeta {
    pub specification: String,
    pub acquire_channel: String,
    pub old_code: Option<String>,
}

impl sqlx::Type<sqlx::Postgres> for ProductMeta {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <serde_json::Value as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ProductMeta {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let val = serde_json::to_value(self)?;
        <serde_json::Value as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&val, buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ProductMeta {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let val = <serde_json::Value as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Ok(serde_json::from_value(val)?)
    }
}

/// 产品实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Product {
    pub product_id: i64,
    pub pdt_name: String,
    pub product_code: String,
    pub unit: String,
    pub status: ProductStatus,
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: ProductMeta,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 创建产品请求
#[derive(Debug, Clone)]
pub struct CreateProductReq {
    pub name: String,
    pub unit: String,
    pub status: ProductStatus,
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: ProductMeta,
}

/// 更新产品请求
#[derive(Debug, Clone, Default)]
pub struct UpdateProductReq {
    pub name: Option<String>,
    pub unit: Option<String>,
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: Option<ProductMeta>,
}

/// 产品查询过滤
#[derive(Debug, Clone, Default)]
pub struct ProductQuery {
    pub name: Option<String>,
    pub code: Option<String>,
    pub status: Option<ProductStatus>,
    pub owner_department_id: Option<i64>,
    pub category_id: Option<i64>,
}

/// 产品使用情况查询
#[derive(Debug, Clone, Default)]
pub struct UsageQuery {
    pub page: u32,
    pub page_size: u32,
}

/// 产品使用条目 — 记录产品在哪些 BOM 中被引用
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UsageEntry {
    pub source_type: String,
    pub source_id: i64,
    pub source_name: String,
    /// BOM 状态: 1=草稿, 2=已发布
    pub bom_status: Option<i16>,
    /// BOM 版本号
    pub bom_version: Option<i32>,
    /// 用量 (来自 bom_nodes.quantity)
    pub quantity: Option<rust_decimal::Decimal>,
    /// 用量单位 (来自 bom_nodes.unit)
    pub node_unit: Option<String>,
    /// 用途备注 (来自 bom_nodes.remark)
    pub node_remark: Option<String>,
    /// 父件产品名称 (BOM 根节点的产品名)
    pub parent_product_name: Option<String>,
    /// 父件产品编码 (BOM 根节点的产品编码)
    pub parent_product_code: Option<String>,
    /// BOM 更新时间
    pub bom_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
