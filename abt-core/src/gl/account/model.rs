use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use super::super::enums::{AccountType, BalanceDirection};

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// 科目表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GlAccount {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub parent_id: Option<i64>,
    pub is_detail: bool,
    pub balance_direction: BalanceDirection,
    pub company_id: i64,
    pub reconcile: bool,
    pub disabled: bool,
    pub opening_balance: Decimal,
    pub currency: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 创建科目请求
#[derive(Debug, Clone)]
pub struct CreateGlAccountReq {
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub parent_id: Option<i64>,
    pub is_detail: bool,
    pub balance_direction: BalanceDirection,
    pub reconcile: bool,
    pub opening_balance: Decimal,
    pub currency: String,
}

/// 更新科目请求
#[derive(Debug, Clone)]
pub struct UpdateGlAccountReq {
    pub name: Option<String>,
    pub disabled: Option<bool>,
    pub version: i32,
}

// ---------------------------------------------------------------------------
// Query filter
// ---------------------------------------------------------------------------

/// 科目查询过滤
#[derive(Debug, Clone, Default)]
pub struct GlAccountFilter {
    pub keyword: Option<String>,
    pub account_type: Option<AccountType>,
    pub disabled: Option<bool>,
}
