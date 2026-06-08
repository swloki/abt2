use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ── TypedPath definitions ──

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms")]
pub struct QmsDashboardPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/specs")]
pub struct SpecListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/specs/table")]
pub struct SpecTablePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/specs/create")]
pub struct SpecCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/specs/{id}")]
pub struct SpecDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/results")]
pub struct ResultListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/results/table")]
pub struct ResultTablePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/results/create")]
pub struct ResultCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/results/{id}")]
pub struct ResultDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/mrb")]
pub struct MrbListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/mrb/table")]
pub struct MrbTablePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/mrb/create")]
pub struct MrbCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/mrb/{id}")]
pub struct MrbDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/rma")]
pub struct RmaListPath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/rma/table")]
pub struct RmaTablePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/rma/create")]
pub struct RmaCreatePath;

#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/qms/rma/{id}")]
pub struct RmaDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        // Dashboard
        .route(QmsDashboardPath::PATH, get(crate::pages::qms_dashboard::get_dashboard))
        // Inspection Specifications
        .route(SpecListPath::PATH, get(crate::pages::qms_spec_list::get_list))
        .route(SpecTablePath::PATH, get(crate::pages::qms_spec_list::get_table))
        .route(SpecCreatePath::PATH, get(crate::pages::qms_spec_create::get_create).post(crate::pages::qms_spec_create::create))
        .route(SpecDetailPath::PATH, get(crate::pages::qms_spec_detail::get_detail))
        // Inspection Results
        .route(ResultListPath::PATH, get(crate::pages::qms_result_list::get_list))
        .route(ResultTablePath::PATH, get(crate::pages::qms_result_list::get_table))
        .route(ResultCreatePath::PATH, get(crate::pages::qms_result_create::get_create).post(crate::pages::qms_result_create::create))
        .route(ResultDetailPath::PATH, get(crate::pages::qms_result_detail::get_detail))
        // MRB
        .route(MrbListPath::PATH, get(crate::pages::qms_mrb_list::get_list))
        .route(MrbTablePath::PATH, get(crate::pages::qms_mrb_list::get_table))
        .route(MrbCreatePath::PATH, get(crate::pages::qms_mrb_create::get_create).post(crate::pages::qms_mrb_create::create))
        .route(MrbDetailPath::PATH, get(crate::pages::qms_mrb_detail::get_detail))
        // RMA
        .route(RmaListPath::PATH, get(crate::pages::qms_rma_list::get_list))
        .route(RmaTablePath::PATH, get(crate::pages::qms_rma_list::get_table))
        .route(RmaCreatePath::PATH, get(crate::pages::qms_rma_create::get_create).post(crate::pages::qms_rma_create::create))
        .route(RmaDetailPath::PATH, get(crate::pages::qms_rma_detail::get_detail))
}
