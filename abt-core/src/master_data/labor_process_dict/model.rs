use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 工序字典实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LaborProcessDict {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
    /// 默认工作中心（选该工序时自动带出）
    pub default_work_center_id: Option<i64>,
    /// 默认标准工时（分钟，选该工序时自动带出）
    pub default_standard_time: Option<Decimal>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 创建工序字典请求
#[derive(Debug, Clone)]
pub struct CreateLaborProcessDictReq {
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub default_work_center_id: Option<i64>,
    pub default_standard_time: Option<Decimal>,
}

/// 更新工序字典请求
#[derive(Debug, Clone, Default)]
pub struct UpdateLaborProcessDictReq {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
    pub default_work_center_id: Option<i64>,
    pub default_standard_time: Option<Decimal>,
}

/// 工序字典查询
#[derive(Debug, Clone, Default)]
pub struct LaborProcessDictQuery {
    pub keyword: Option<String>,
}
