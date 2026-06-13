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
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
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

/// BOM 实体 — UML v4 Bom
/// 注意：不使用 sqlx::FromRow，因为 bom_detail 从 bom_nodes 表加载，需要 repo 层手动映射
#[derive(Debug, Clone)]
pub struct Bom {
    pub bom_id: i64,
    pub bom_name: String,
    pub create_at: DateTime<Utc>,
    pub update_at: Option<DateTime<Utc>>,
    pub bom_detail: BomDetail,
    pub bom_category_id: Option<i64>,
    pub status: BomStatus,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
}

/// BOM 明细 — 设计定义为节点向量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDetail {
    pub nodes: Vec<BomNode>,
}

impl BomDetail {
    /// 提取叶子节点（没有子节点的节点）
    pub fn leaf_nodes(&self) -> Vec<&BomNode> {
        let parent_ids: std::collections::HashSet<i64> =
            self.nodes.iter().map(|n| n.parent_id).collect();
        self.nodes
            .iter()
            .filter(|n| !parent_ids.contains(&n.id))
            .collect()
    }
}

impl sqlx::Type<sqlx::Postgres> for BomDetail {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <serde_json::Value as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for BomDetail {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let val = serde_json::to_value(self)?;
        <serde_json::Value as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&val, buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for BomDetail {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let val = <serde_json::Value as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Ok(serde_json::from_value(val)?)
    }
}

/// BOM 节点 (树结构) — UML v4 BomNode
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct BomNode {
    #[serde(rename = "node_id")]
    #[sqlx(rename = "node_id")]
    pub id: i64,
    pub bom_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub quantity: Decimal,
    pub parent_id: i64,
    pub loss_rate: Decimal,
    #[serde(rename = "order_num")]
    #[sqlx(rename = "order_num")]
    pub order: i32,
    pub unit: Option<String>,
    pub remark: Option<String>,
    pub position: Option<String>,
    pub work_center: Option<String>,
    pub properties: Option<String>,
}

/// BOM 快照 (已发布版本)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomSnapshot {
    pub snapshot_id: i64,
    pub bom_id: i64,
    pub version: i32,
    pub bom_name: String,
    pub bom_detail: BomDetail,
    pub published_at: DateTime<Utc>,
    pub published_by: i64,
}

/// BOM 分类 — UML v4: 仅 3 字段 (bom_category_id, bom_category_name, created_at)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomCategory {
    pub bom_category_id: i64,
    pub bom_category_name: String,
    pub created_at: DateTime<Utc>,
}

/// 成本报告
#[derive(Debug, Clone)]
pub struct BomCostReport {
    pub bom_id: i64,
    pub bom_name: String,
    pub product_code: String,
    pub as_of_date: Option<DateTime<Utc>>,
    pub material_costs: Vec<MaterialCostItem>,
    pub labor_costs: Vec<LaborCostItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MaterialCostItem {
    pub node_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: Decimal,
    pub unit_price: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct LaborCostItem {
    pub id: i64,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: String,
}

#[derive(Debug, Clone)]
pub struct BomLaborCostReport {
    pub bom_id: i64,
    pub items: Vec<LaborCostItem>,
    pub total_cost: Decimal,
}

// ---- Request / Response structs ----

#[derive(Debug, Clone)]
pub struct BomQuery {
    pub name: Option<String>,
    pub status: Option<BomStatus>,
    pub bom_category_id: Option<i64>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateBomReq {
    pub name: String,
    pub bom_category_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBomReq {
    pub name: Option<String>,
    pub bom_category_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewBomNode {
    pub product_id: i64,
    pub quantity: Decimal,
    pub parent_id: i64,
    pub loss_rate: Decimal,
    pub order: i32,
    pub unit: Option<String>,
    pub remark: Option<String>,
    pub position: Option<String>,
    pub work_center: Option<String>,
    pub properties: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBomNodeReq {
    pub quantity: Option<Decimal>,
    pub loss_rate: Option<Decimal>,
    pub order: Option<i32>,
    pub unit: Option<String>,
    pub remark: Option<String>,
    pub position: Option<String>,
    pub work_center: Option<String>,
    pub properties: Option<String>,
}

/// 属性覆盖 — 替换物料时可选择性覆盖节点属性
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttributeOverrides {
    pub quantity: Option<Decimal>,
    pub loss_rate: Option<Decimal>,
    pub unit: Option<String>,
    pub remark: Option<String>,
    pub position: Option<String>,
    pub work_center: Option<String>,
    pub properties: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubstituteReq {
    pub old_product_id: i64,
    pub new_product_id: i64,
    pub bom_id: Option<i64>,
    pub overrides: AttributeOverrides,
}

#[derive(Debug, Clone)]
pub struct SubstitutionResult {
    pub affected_boms: i64,
    pub affected_nodes: i64,
}

#[derive(Debug, Clone)]
pub struct CreateBomCategoryReq {
    pub bom_category_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBomCategoryReq {
    pub bom_category_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BomCategoryQuery {
    pub name: Option<String>,
}
