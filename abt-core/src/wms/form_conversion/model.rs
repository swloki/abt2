use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::{ConversionDir, ConversionStatus};

/// 形态转换单实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FormConversion {
    pub id: i64,
    pub doc_number: String,
    pub warehouse_id: i64,
    pub conversion_date: NaiveDate,
    pub status: ConversionStatus,
    pub remark: String,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

/// 形态转换单行项目
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversionItem {
    pub id: i64,
    pub conversion_id: i64,
    pub direction: ConversionDir,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub batch_no: Option<String>,
}

/// 创建形态转换单请求
#[derive(Debug, Clone)]
pub struct CreateConversionReq {
    pub warehouse_id: i64,
    pub conversion_date: NaiveDate,
    pub remark: String,
    pub items: Vec<CreateConversionItemReq>,
}

/// 创建形态转换单行项目请求
#[derive(Debug, Clone)]
pub struct CreateConversionItemReq {
    pub direction: ConversionDir,
    pub product_id: i64,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub batch_no: Option<String>,
}

/// 形态转换单查询过滤
#[derive(Debug, Clone, Default)]
pub struct ConversionFilter {
    pub status: Option<ConversionStatus>,
    pub warehouse_id: Option<i64>,
}
