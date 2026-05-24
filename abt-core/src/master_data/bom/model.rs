use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// BOM 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum BomStatus {
    Draft = 1,
    Published = 2,
}

impl BomStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Published),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for BomStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for BomStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for BomStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown BomStatus: {v}").into())
    }
}

impl serde::Serialize for BomStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for BomStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown BomStatus: {v}")))
    }
}

/// BOM 实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Bom {
    pub bom_id: i64,
    pub bom_name: String,
    pub bom_code: String,
    pub version: i32,
    pub status: BomStatus,
    pub category_id: Option<i64>,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// BOM 明细 (JSONB stored in bom_detail column)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDetail {
    pub total_material_cost: Option<Decimal>,
    pub total_labor_cost: Option<Decimal>,
    pub custom_fields: Option<serde_json::Value>,
}

/// BOM 节点 (树结构)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomNode {
    pub node_id: i64,
    pub bom_id: i64,
    pub parent_node_id: Option<i64>,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit: String,
    pub order_num: i32,
    pub attr_overrides: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// BOM 快照 (已发布版本)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomSnapshot {
    pub snapshot_id: i64,
    pub bom_id: i64,
    pub version: i32,
    pub snapshot_data: serde_json::Value,
    pub created_at: Option<DateTime<Utc>>,
}

/// BOM 分类
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomCategory {
    pub category_id: i64,
    pub category_name: String,
    pub remark: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 成本报告类型
#[derive(Debug, Clone)]
pub struct BomCostReport {
    pub bom_id: i64,
    pub bom_name: String,
    pub total_material_cost: Decimal,
    pub total_labor_cost: Decimal,
    pub material_items: Vec<MaterialCostItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MaterialCostItem {
    pub node_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
}

// ---- Request / Response structs ----

#[derive(Debug, Clone)]
pub struct BomQuery {
    pub name: Option<String>,
    pub status: Option<BomStatus>,
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CreateBomReq {
    pub bom_name: String,
    pub category_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBomReq {
    pub bom_name: Option<String>,
    pub category_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewBomNode {
    pub parent_node_id: Option<i64>,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit: String,
    pub order_num: Option<i32>,
    pub attr_overrides: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBomNodeReq {
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    pub order_num: Option<i32>,
    pub attr_overrides: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SubstituteReq {
    pub bom_id: i64,
    pub old_product_id: i64,
    pub new_product_id: i64,
}

#[derive(Debug, Clone)]
pub struct SubstitutionResult {
    pub affected_count: i64,
    pub affected_node_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct CreateBomCategoryReq {
    pub category_name: String,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBomCategoryReq {
    pub category_name: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BomCategoryQuery {
    pub name: Option<String>,
}
