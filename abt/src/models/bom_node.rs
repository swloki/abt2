//! BOM 节点数据模型（映射 bom_nodes 表）

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sqlx::FromRow;

/// f64 → Decimal 转换辅助函数
pub fn f64_to_decimal(v: f64) -> Decimal {
    Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO)
}

/// bom_nodes 表行映射
#[derive(Debug, Clone, FromRow)]
pub struct BomNodeRow {
    pub id: i64,
    pub bom_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub quantity: Decimal,
    pub parent_id: Option<i64>,
    pub loss_rate: Decimal,
    pub order: i32,
    pub unit: Option<String>,
    pub remark: Option<String>,
    pub position: Option<String>,
    pub work_center: Option<String>,
    pub properties: Option<String>,
}

impl From<BomNodeRow> for super::BomNode {
    fn from(row: BomNodeRow) -> Self {
        Self {
            id: row.id,
            product_id: row.product_id,
            product_code: row.product_code,
            quantity: row.quantity.to_f64().unwrap_or(0.0),
            parent_id: row.parent_id.unwrap_or(0),
            loss_rate: row.loss_rate.to_f64().unwrap_or(0.0),
            order: row.order,
            unit: row.unit,
            remark: row.remark,
            position: row.position,
            work_center: row.work_center,
            properties: row.properties,
        }
    }
}

/// 插入用的新节点（无 id，由数据库生成）
#[derive(Debug, Clone)]
pub struct NewBomNode {
    pub bom_id: i64,
    pub product_id: i64,
    pub product_code: Option<String>,
    pub quantity: Decimal,
    pub parent_id: Option<i64>,
    pub loss_rate: Decimal,
    pub order: i32,
    pub unit: Option<String>,
    pub remark: Option<String>,
    pub position: Option<String>,
    pub work_center: Option<String>,
    pub properties: Option<String>,
}

impl NewBomNode {
    pub fn from_node(bom_id: i64, order: i32, node: &super::BomNode) -> Self {
        Self {
            bom_id,
            product_id: node.product_id,
            product_code: node.product_code.clone(),
            quantity: f64_to_decimal(node.quantity),
            parent_id: if node.parent_id == 0 { None } else { Some(node.parent_id) },
            loss_rate: f64_to_decimal(node.loss_rate),
            order,
            unit: node.unit.clone(),
            remark: node.remark.clone(),
            position: node.position.clone(),
            work_center: node.work_center.clone(),
            properties: node.properties.clone(),
        }
    }
}
