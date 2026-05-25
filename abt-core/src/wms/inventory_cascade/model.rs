use rust_decimal::Decimal;

/// BOM 展开库存查询结果
#[derive(Debug, Clone)]
pub struct CascadeInventoryResult {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub bom_groups: Vec<BomCascadeGroup>,
}

/// BOM 分组（一个产品可能有多个 BOM）
#[derive(Debug, Clone)]
pub struct BomCascadeGroup {
    pub bom_id: i64,
    pub bom_name: String,
    pub children: Vec<ChildNodeInventory>,
}

/// 子件库存信息
#[derive(Debug, Clone)]
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

/// BOM 展开查询请求
#[derive(Debug, Clone, Default)]
pub struct CascadeInventoryQuery {
    pub product_id: Option<i64>,
    pub product_code: Option<String>,
    /// 限制返回节点数（防大 BOM 爆炸）
    pub max_results: i32,
}
