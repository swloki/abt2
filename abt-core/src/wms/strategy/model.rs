use crate::wms::enums::{PickType, PutawayType};

/// 上架策略实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PutawayStrategy {
    pub id: i64,
    pub name: String,
    pub strategy_type: PutawayType,
    pub warehouse_id: Option<i64>,
    pub product_category_id: Option<i64>,
    pub priority: i32,
    pub is_active: bool,
}

/// 拣货策略实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PickStrategy {
    pub id: i64,
    pub name: String,
    pub strategy_type: PickType,
    pub warehouse_id: Option<i64>,
    pub priority: i32,
    pub is_active: bool,
}

/// 创建策略请求（通用，caller 将 strategy_type_val 转为对应枚举）
#[derive(Debug, Clone)]
pub struct CreateStrategyReq {
    pub name: String,
    pub strategy_type_val: i16,
    pub warehouse_id: Option<i64>,
    pub priority: Option<i32>,
}
