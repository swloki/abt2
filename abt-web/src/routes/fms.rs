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
#[typed_path("/admin/fms/journals/create")]
pub struct JournalCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/{id}")]
pub struct JournalDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/{id}/confirm")]
pub struct JournalConfirmPath {
    pub id: i64,
}

// Write-Off
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/writeoffs")]
pub struct WriteoffListPath;

// Cost Analysis
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/cost-analysis")]
pub struct CostAnalysisPath;

// AR/AP Ledger
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ar-ledger")]
pub struct ArLedgerPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ap-ledger")]
pub struct ApLedgerPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/ap-ledger/search-supplier")]
pub struct ApSupplierSearchPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/ar-ledger/search-customer")]
pub struct ArCustomerSearchPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ar-ledger/detail")]
pub struct ArLedgerDetailPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ap-ledger/detail")]
pub struct ApLedgerDetailPath;

// AR/AP Adjustment
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ar-adjustments")]
pub struct ArAdjustmentListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ar-adjustments/create")]
pub struct ArAdjustmentCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ap-adjustments")]
pub struct ApAdjustmentListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ap-adjustments/create")]
pub struct ApAdjustmentCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/adjustments/balance")]
pub struct AdjustmentBalancePath;

// AR/AP Aging
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ar-aging")]
pub struct ArAgingPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ap-aging")]
pub struct ApAgingPath;

// Settlement
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/settlement")]
pub struct SettlementListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/settlement/{id}/unsettle")]
pub struct SettlementUnsettlePath {
    pub id: i64,
}

// Journal search endpoints (for entity_picker)
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/search-counterparty")]
pub struct JournalSearchCpPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/journals/search-account")]
pub struct JournalSearchAccountPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        // Dashboard
        .route(FmsDashboardPath::PATH, get(crate::pages::fms_dashboard::get_dashboard))
        // Cash Journal
        .route(JournalListPath::PATH, get(crate::pages::fms_journal_list::get_list))
.route(JournalCreatePath::PATH, get(crate::pages::fms_journal_create::get_create).post(crate::pages::fms_journal_create::create))
        .route(JournalDetailPath::PATH, get(crate::pages::fms_journal_detail::get_detail))
        .route(JournalConfirmPath::PATH, axum::routing::post(crate::pages::fms_journal_detail::confirm))
        .route(JournalSearchCpPath::PATH, get(crate::pages::fms_journal_create::search_counterparty))
        .route(JournalSearchAccountPath::PATH, get(crate::pages::fms_journal_create::search_account))
        // Write-Off
        .route(WriteoffListPath::PATH, get(crate::pages::fms_writeoff_list::get_list))
// Cost Analysis
        .route(CostAnalysisPath::PATH, get(crate::pages::fms_cost_analysis::get_page))
        // AR/AP Ledger
        .route(ArLedgerPath::PATH, get(crate::pages::fms_ar_ledger::get_list))
        .route(ApLedgerPath::PATH, get(crate::pages::fms_ap_ledger::get_list))
        .route(ArLedgerDetailPath::PATH, get(crate::pages::fms_ar_ledger::get_detail))
        .route(ApLedgerDetailPath::PATH, get(crate::pages::fms_ap_ledger::get_detail))
        .route(ApSupplierSearchPath::PATH, get(crate::pages::fms_ap_ledger::search_supplier))
        .route(ArCustomerSearchPath::PATH, get(crate::pages::fms_ar_ledger::search_customer))
        // AR/AP Adjustment
        .route(ArAdjustmentListPath::PATH, get(crate::pages::fms_adjustment_list::get_ar_list))
        .route(ArAdjustmentCreatePath::PATH, get(crate::pages::fms_adjustment_create::get_ar_create).post(crate::pages::fms_adjustment_create::create_ar))
        .route(ApAdjustmentListPath::PATH, get(crate::pages::fms_adjustment_list::get_ap_list))
        .route(ApAdjustmentCreatePath::PATH, get(crate::pages::fms_adjustment_create::get_ap_create).post(crate::pages::fms_adjustment_create::create_ap))
        .route(AdjustmentBalancePath::PATH, get(crate::pages::fms_adjustment_create::get_balance))
        // AR/AP Aging
        .route(ArAgingPath::PATH, get(crate::pages::fms_ar_aging::get_page))
        .route(ApAgingPath::PATH, get(crate::pages::fms_ap_aging::get_page))
        // Settlement
        .route(SettlementListPath::PATH, get(crate::pages::fms_settlement::get_list))
        .route(SettlementUnsettlePath::PATH, axum::routing::post(crate::pages::fms_settlement::unsettle))
}
