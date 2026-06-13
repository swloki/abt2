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

/// 产品获取途径
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum AcquireChannel {
    SelfProduced = 1,  // 自制
    Purchased = 2,     // 外购
    Outsourced = 3,    // 委外（预留）
    NonInventory = 4,  // 费用/服务/虚拟件（跳过库存校验和补货）
    Legacy = 9,        // 历史遗留（行为等同自制，日志驱动数据清洗）
}

impl AcquireChannel {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::SelfProduced),
            2 => Some(Self::Purchased),
            3 => Some(Self::Outsourced),
            4 => Some(Self::NonInventory),
            9 => Some(Self::Legacy),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfProduced => "SelfProduced",
            Self::Purchased => "Purchased",
            Self::Outsourced => "Outsourced",
            Self::NonInventory => "NonInventory",
            Self::Legacy => "Legacy",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for AcquireChannel {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for AcquireChannel {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for AcquireChannel {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown AcquireChannel: {v}").into())
    }
}

impl serde::Serialize for AcquireChannel {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for AcquireChannel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown AcquireChannel: {v}")))
    }
}

/// 物料消耗策略
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialConsumptionMode {
    /// 倒冲模式（默认）：完工时按 BOM 自动扣减原材料
    #[default]
    #[serde(rename = "backflush")]
    Backflush,
    /// 领料模式：release 时生成领料单，手动领料出库
    #[serde(rename = "picking")]
    Picking,
}

/// 超额完工容差默认值（5%）—— 单一事实来源，供 serde 默认与运行时回退共用
pub fn default_over_completion_tolerance() -> rust_decimal::Decimal {
    rust_decimal::Decimal::from_str_exact("0.05")
        .expect("0.05 is a valid decimal literal")
}

fn default_tolerance() -> Option<rust_decimal::Decimal> {
    Some(default_over_completion_tolerance())
}

/// 产品元数据 (JSONB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductMeta {
    pub specification: String,
    /// acquire_channel 已迁移为 Product 独立列
    pub old_code: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    /// 物料消耗策略：backflush（默认）或 picking
    #[serde(default)]
    pub material_consumption_mode: MaterialConsumptionMode,
    /// 超额完工容差百分比（默认 5%）
    #[serde(default = "default_tolerance")]
    pub over_completion_tolerance: Option<rust_decimal::Decimal>,
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
    pub acquire_channel: AcquireChannel,
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
    pub acquire_channel: AcquireChannel,
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: ProductMeta,
}

/// 更新产品请求
#[derive(Debug, Clone, Default)]
pub struct UpdateProductReq {
    pub name: Option<String>,
    pub unit: Option<String>,
    pub acquire_channel: Option<AcquireChannel>,
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
