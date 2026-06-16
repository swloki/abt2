use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::purchase::enums::TaxType;

/// 税率实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaxRate {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub rate: Decimal,
    pub tax_type: TaxType,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
