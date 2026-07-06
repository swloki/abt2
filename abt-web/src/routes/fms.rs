use axum::response::Redirect;
use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ── TypedPath definitions ──

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

// Cost Analysis
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/cost-analysis")]
pub struct CostAnalysisPath;

// AR/AP Adjustment
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/fms/ar-adjustments/create")]
pub struct ArAdjustmentCreatePath;

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
        // /admin/fms 根路径重定向到财务作业中心（原 dashboard 已移除）
        .route("/admin/fms", get(|| async { Redirect::permanent("/admin/fms/work-center") }))
        // Cash Journal
        .route(JournalListPath::PATH, get(crate::pages::fms_journal_list::get_list))
.route(JournalCreatePath::PATH, get(crate::pages::fms_journal_create::get_create).post(crate::pages::fms_journal_create::create))
        .route(JournalDetailPath::PATH, get(crate::pages::fms_journal_detail::get_detail))
        .route(JournalConfirmPath::PATH, axum::routing::post(crate::pages::fms_journal_detail::confirm))
        .route(JournalSearchCpPath::PATH, get(crate::pages::fms_journal_create::search_counterparty))
        .route(JournalSearchAccountPath::PATH, get(crate::pages::fms_journal_create::search_account))
        // Cost Analysis
        .route(CostAnalysisPath::PATH, get(crate::pages::fms_cost_analysis::get_page))
        // AR/AP Adjustment
        .route(ArAdjustmentCreatePath::PATH, get(crate::pages::fms_adjustment_create::get_ar_create).post(crate::pages::fms_adjustment_create::create_ar))
        .route(ApAdjustmentCreatePath::PATH, get(crate::pages::fms_adjustment_create::get_ap_create).post(crate::pages::fms_adjustment_create::create_ap))
        .route(AdjustmentBalancePath::PATH, get(crate::pages::fms_adjustment_create::get_balance))
        // AR/AP Aging
        .route(ArAgingPath::PATH, get(crate::pages::fms_ar_aging::get_page))
        .route(ApAgingPath::PATH, get(crate::pages::fms_ap_aging::get_page))
}
