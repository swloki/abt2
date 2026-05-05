//! 级联查询库存模型

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 级联查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeInventoryResult {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub bom_groups: Vec<BomCascadeGroup>,
}

/// 按 BOM 分组的级联数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomCascadeGroup {
    pub bom_id: i64,
    pub bom_name: String,
    pub children: Vec<ChildNodeInventory>,
}

/// 子节点库存信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildNodeInventory {
    pub node_id: i64,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub total_stock: Decimal,
    pub loss_rate: Decimal,
    pub order: i32,
    pub parent_node_id: Option<i64>,
}
