use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::super::enums::{ExpenseStatus, ExpenseType};

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// 报销单主实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseReimbursement {
    pub id: i64,
    pub doc_number: String,
    pub applicant_id: i64,
    pub department_id: Option<i64>,
    pub expense_date: NaiveDate,
    pub total_amount: Decimal,
    pub status: ExpenseStatus,
    pub remark: String,
    pub operator_id: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// 报销单明细实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseReimbursementItem {
    pub id: i64,
    pub reimbursement_id: i64,
    pub expense_type: ExpenseType,
    pub amount: Decimal,
    pub description: String,
    pub receipt_no: Option<String>,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
}

// ---------------------------------------------------------------------------
// Request / Input types
// ---------------------------------------------------------------------------

/// 创建报销单请求
#[derive(Debug, Clone)]
pub struct CreateExpenseReq {
    pub applicant_id: i64,
    pub department_id: Option<i64>,
    pub expense_date: NaiveDate,
    pub remark: String,
    pub items: Vec<ExpenseItemInput>,
}

/// 报销单明细输入
#[derive(Debug, Clone)]
pub struct ExpenseItemInput {
    pub expense_type: ExpenseType,
    pub amount: Decimal,
    pub description: String,
    pub receipt_no: Option<String>,
    pub cost_center: Option<i64>,
    pub profit_center: Option<i64>,
}

// ---------------------------------------------------------------------------
// Query filter
// ---------------------------------------------------------------------------

/// 报销单查询过滤
#[derive(Debug, Clone, Default)]
pub struct ExpenseFilter {
    pub status: Vec<ExpenseStatus>,
    pub applicant_id: Option<i64>,
    pub department_id: Option<i64>,
    pub expense_date_from: Option<NaiveDate>,
    pub expense_date_to: Option<NaiveDate>,
}
