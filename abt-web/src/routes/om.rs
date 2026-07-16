use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{om_dashboard, om_outsourcing_list, om_outsourcing_create, om_outsourcing_detail, om_tracking_list};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om")]
pub struct OmDashboardPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing")]
pub struct OmOutsourcingListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/create")]
pub struct OmOutsourcingCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/{id}")]
pub struct OmOutsourcingDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/{id}/receive")]
pub struct OmOutsourcingReceivePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/{id}/convert")]
pub struct OmOutsourcingConvertPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/{id}/cancel")]
pub struct OmOutsourcingCancelPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/{id}/record-node")]
pub struct OmRecordNodePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/wo-summary")]
pub struct OmOutsourcingWoSummaryPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/outsourcing/suggest-materials")]
pub struct OmOutsourcingSuggestMaterialsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/om/tracking")]
pub struct OmTrackingListPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        // Dashboard
        .route(OmDashboardPath::PATH, get(om_dashboard::get_dashboard))
        // Outsourcing CRUD
        .route(OmOutsourcingListPath::PATH, get(om_outsourcing_list::get_list))
.route(
            OmOutsourcingCreatePath::PATH,
            get(om_outsourcing_create::get_create).post(om_outsourcing_create::create),
        )
        .route(OmOutsourcingDetailPath::PATH, get(om_outsourcing_detail::get_detail))
        .route(OmOutsourcingReceivePath::PATH, post(om_outsourcing_detail::receive_order))
        .route(OmOutsourcingConvertPath::PATH, post(om_outsourcing_detail::convert_to_internal))
        .route(OmOutsourcingCancelPath::PATH, post(om_outsourcing_detail::cancel_order))
        .route(OmRecordNodePath::PATH, post(om_outsourcing_detail::record_node))
        .route(OmOutsourcingWoSummaryPath::PATH, get(om_outsourcing_create::wo_summary))
        .route(OmOutsourcingSuggestMaterialsPath::PATH, get(om_outsourcing_create::suggest_materials))
        // Tracking
        .route(OmTrackingListPath::PATH, get(om_tracking_list::get_list))
}
