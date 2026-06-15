use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 采购审批规则实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PurchaseApprovalRule {
    pub id: i64,
    pub name: String,
    pub min_amount: Decimal,
    pub max_amount: Option<Decimal>,
    pub approver_role: String,
    pub approver_id: Option<i64>,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
