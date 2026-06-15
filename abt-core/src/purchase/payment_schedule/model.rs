use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

/// 付款计划实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaymentSchedule {
    pub id: i64,
    pub order_id: i64,
    pub line_no: i32,
    pub due_date: NaiveDate,
    pub payment_pct: Decimal,
    pub payment_amount: Decimal,
    pub paid_amount: Decimal,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 付款计划输入（生成时使用）
#[derive(Debug, Clone)]
pub struct PaymentScheduleInput {
    pub due_date: NaiveDate,
    pub payment_pct: Decimal,
    pub description: String,
}
