use rust_decimal::Decimal;

/// 产品成本汇总
#[derive(Debug, Clone)]
pub struct ProductCostSummary {
    pub product_id: i64,
    pub period: String,
    pub material_cost: Decimal,
    pub labor_cost: Decimal,
    pub overhead_cost: Decimal,
    pub outsource_cost: Decimal,
    pub rework_cost: Decimal,
    pub scrap_cost: Decimal,
    pub total_cost: Decimal,
}

/// 工单成本汇总
#[derive(Debug, Clone)]
pub struct WorkOrderCostSummary {
    pub work_order_id: i64,
    pub material_cost: Decimal,
    pub labor_cost: Decimal,
    pub overhead_cost: Decimal,
    pub total_cost: Decimal,
}

/// 利润中心汇总
#[derive(Debug, Clone)]
pub struct ProfitCenterSummary {
    pub profit_center_id: i64,
    pub period: String,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub net_amount: Decimal,
}

/// 毛利分析
#[derive(Debug, Clone)]
pub struct MarginAnalysis {
    pub order_id: i64,
    pub estimated_cost: Decimal,
    pub actual_cost: Decimal,
    pub margin_amount: Decimal,
    pub margin_rate: Decimal,
}

/// 成本类型汇总行（repo 内部使用，映射 GROUP BY cost_type 的结果）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CostTypeRow {
    pub cost_type: i16,
    pub total: Decimal,
}

/// 利润中心汇总行（repo 内部使用，映射 GROUP BY profit_center, period 的结果）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfitCenterRow {
    pub profit_center: i64,
    pub period: String,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
}

/// 产品成本汇总行（含产品信息，用于列表展示）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductCostRow {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub cost_type: i16,
    pub total: Decimal,
}

/// 工单成本汇总行（含工单+产品信息）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkOrderCostRow {
    pub work_order_id: i64,
    pub doc_number: String,
    pub product_name: String,
    pub planned_qty: Decimal,
    pub completed_qty: Option<Decimal>,
    pub wo_status: i16,
    pub cost_type: i16,
    pub total: Decimal,
}

/// 利润中心 P&L 行（含部门名，按 profit_center + period 分组，每种 cost_type 各一行）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfitCenterPLRow {
    pub profit_center: i64,
    pub period: String,
    pub cost_type: i16,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
}

/// 毛利分析行（含订单+客户信息）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MarginRow {
    pub order_id: i64,
    pub doc_number: String,
    pub customer_name: String,
    pub order_amount: Decimal,
    pub cost_type: i16,
    pub total_cost: Decimal,
}
