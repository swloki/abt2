use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ── TypedPath definitions ──

// Dashboard
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms")]
pub struct FmsDashboardPath;

// Cash Journal
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals")]
pub struct JournalListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/table")]
pub struct JournalTablePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/create")]
pub struct JournalCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/{id}")]
pub struct JournalDetailPath {
    pub id: i64,
}

// Expense Reimbursement
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/expenses")]
pub struct ExpenseListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/expenses/table")]
pub struct ExpenseTablePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/expenses/create")]
pub struct ExpenseCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/expenses/{id}")]
pub struct ExpenseDetailPath {
    pub id: i64,
}

// Write-Off
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/writeoffs")]
pub struct WriteoffListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/writeoffs/table")]
pub struct WriteoffTablePath;

// Cost Analysis
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/cost-analysis")]
pub struct CostAnalysisPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        // Dashboard
        .route(FmsDashboardPath::PATH, get(crate::pages::fms_dashboard::get_dashboard))
        // Cash Journal
        .route(JournalListPath::PATH, get(crate::pages::fms_journal_list::get_list))
        .route(JournalTablePath::PATH, get(crate::pages::fms_journal_list::get_table))
        .route(JournalCreatePath::PATH, get(crate::pages::fms_journal_create::get_create).post(crate::pages::fms_journal_create::create))
        .route(JournalDetailPath::PATH, get(crate::pages::fms_journal_detail::get_detail))
        // Expense
        .route(ExpenseListPath::PATH, get(crate::pages::fms_expense_list::get_list))
        .route(ExpenseTablePath::PATH, get(crate::pages::fms_expense_list::get_table))
        .route(ExpenseCreatePath::PATH, get(crate::pages::fms_expense_create::get_create).post(crate::pages::fms_expense_create::create))
        .route(ExpenseDetailPath::PATH, get(crate::pages::fms_expense_detail::get_detail))
        // Write-Off
        .route(WriteoffListPath::PATH, get(crate::pages::fms_writeoff_list::get_list))
        .route(WriteoffTablePath::PATH, get(crate::pages::fms_writeoff_list::get_table))
        // Cost Analysis
        .route(CostAnalysisPath::PATH, get(crate::pages::fms_cost_analysis::get_page))
}
