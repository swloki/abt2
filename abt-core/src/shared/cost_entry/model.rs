use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::shared::enums::{CostEntityType, CostType, DocumentType};

/// 成本分录实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CostEntry {
    pub id: i64,
    pub entity_type: CostEntityType,
    pub entity_id: i64,
    pub cost_type: CostType,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub period: String,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub created_at: DateTime<Utc>,
}

/// 创建成本分录请求
#[derive(Debug, Clone)]
pub struct EntryRequest {
    pub entity_type: CostEntityType,
    pub entity_id: i64,
    pub cost_type: CostType,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
    pub period: String,
    pub source_type: DocumentType,
    pub source_id: i64,
}
