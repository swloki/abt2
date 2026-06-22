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
    // ── 新增字段 (Issue #63) ──
    pub sheet_count: i32,
    pub has_invoice: bool,
    pub payment_remark: Option<String>,
    pub payment_bank: Option<String>,
    pub payment_date: Option<NaiveDate>,
    pub supervisor_id: Option<i64>,
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
    // ── 新增字段 (Issue #63) ──
    pub occurrence_date: Option<NaiveDate>,
    pub has_invoice: bool,
}

// ---------------------------------------------------------------------------
// Attachment
// ---------------------------------------------------------------------------

/// 报销凭证附件实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseAttachment {
    pub id: i64,
    pub expense_id: i64,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i32,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
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
    // ── 新增字段 ──
    pub sheet_count: i32,
    pub has_invoice: bool,
    pub attachments: Vec<CreateAttachmentReq>,
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
    // ── 新增字段 ──
    pub occurrence_date: Option<NaiveDate>,
    pub has_invoice: bool,
}

/// 创建附件请求
#[derive(Debug, Clone)]
pub struct CreateAttachmentReq {
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i32,
    pub sort_order: i32,
}

/// 直属上级审批请求
#[derive(Debug, Clone)]
pub struct SupervisorApproveReq {
    pub remark: Option<String>,
}

/// 财务审批请求
#[derive(Debug, Clone)]
pub struct FinanceApproveReq {
    pub remark: Option<String>,
}

/// 出纳付款请求（付款信息留痕）
#[derive(Debug, Clone)]
pub struct PayReq {
    pub payment_bank: String,
    pub payment_remark: String,
    pub payment_date: NaiveDate,
}

// ---------------------------------------------------------------------------
// Approval Progress（审批进度）
// ---------------------------------------------------------------------------

/// 审批进度节点（给前端展示）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalProgressNode {
    pub stage: String,
    pub label: String,
    pub status: String, // "completed" | "current" | "pending"
    pub operator_name: Option<String>,
    pub operated_at: Option<String>,
    pub remark: Option<String>,
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
