use chrono::{DateTime, Utc};

/// 利润中心主数据（target.md #6：成本核算 P&L 按利润中心归集）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfitCenter {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub department_id: Option<i64>,
    pub is_active: bool,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateProfitCenterReq {
    pub code: String,
    pub name: String,
    pub department_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateProfitCenterReq {
    pub name: Option<String>,
    pub department_id: Option<i64>,
    pub is_active: Option<bool>,
}
