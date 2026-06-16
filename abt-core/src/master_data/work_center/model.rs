use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 工作中心实体（对标 Odoo mrp.workcenter）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkCenter {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub work_center_type: i16, // 1=机器 2=人工 3=委外
    pub costs_hour: Decimal,
    pub time_efficiency: Decimal,
    pub setup_time: Decimal,
    pub cleanup_time: Decimal,
    pub default_capacity: Decimal,
    pub calendar_id: Option<i64>,
    pub location: Option<String>,
    pub is_active: bool,
    pub operator_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkCenterReq {
    pub code: String,
    pub name: String,
    pub work_center_type: i16,
    pub costs_hour: Decimal,
    pub time_efficiency: Decimal,
    pub setup_time: Decimal,
    pub cleanup_time: Decimal,
    pub default_capacity: Decimal,
    pub calendar_id: Option<i64>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateWorkCenterReq {
    pub name: Option<String>,
    pub work_center_type: Option<i16>,
    pub costs_hour: Option<Decimal>,
    pub time_efficiency: Option<Decimal>,
    pub setup_time: Option<Decimal>,
    pub cleanup_time: Option<Decimal>,
    pub default_capacity: Option<Decimal>,
    pub calendar_id: Option<i64>,
    pub location: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkCenterFilter {
    pub keyword: Option<String>,
    pub work_center_type: Option<i16>,
    pub is_active: Option<bool>,
}

/// 工作中心类型标签（与 work_center_type 字段映射一致：1=机器 2=人工 3=委外）
pub fn work_center_type_label(t: i16) -> &'static str {
    match t {
        1 => "机器",
        2 => "人工",
        3 => "委外",
        _ => "—",
    }
}
