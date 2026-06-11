use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_inspection_list, mes_inspection_create, mes_inspection_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/inspections")]
pub struct InspectionListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/inspections/create")]
pub struct InspectionCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/inspections/{id}")]
pub struct InspectionDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/inspections/{inspection_id}/record-result")]
pub struct InspectionRecordResultPath {
    pub inspection_id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(InspectionListPath::PATH, get(mes_inspection_list::get_inspection_list))
        .route(
            InspectionCreatePath::PATH,
            get(mes_inspection_create::get_inspection_create).post(mes_inspection_create::create_inspection),
        )
        .route(InspectionDetailPath::PATH, get(mes_inspection_detail::get_inspection_detail))
        .route(
            InspectionRecordResultPath::PATH,
            post(mes_inspection_detail::record_result),
        )
}
