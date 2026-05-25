use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// BOM 劳务工序实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BomLaborProcess {
    pub id: i64,
    pub product_code: String,
    pub labor_process_dict_id: i64,
    pub process_code: Option<String>,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
    pub operator_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 创建 BOM 劳务工序请求
#[derive(Debug, Clone)]
pub struct CreateBomLaborProcessReq {
    pub product_code: String,
    pub labor_process_dict_id: i64,
    pub process_code: Option<String>,
    pub name: String,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub sort_order: i32,
    pub remark: Option<String>,
}

/// 更新 BOM 劳务工序请求
#[derive(Debug, Clone, Default)]
pub struct UpdateBomLaborProcessReq {
    pub labor_process_dict_id: Option<i64>,
    pub process_code: Option<String>,
    pub name: Option<String>,
    pub unit_price: Option<Decimal>,
    pub quantity: Option<Decimal>,
    pub sort_order: Option<i32>,
    pub remark: Option<String>,
}

/// BOM 劳务工序查询
#[derive(Debug, Clone, Default)]
pub struct BomLaborProcessQuery {
    pub product_code: Option<String>,
    pub keyword: Option<String>,
}
