use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_cycle_count_list, wms_cycle_count_create, wms_cycle_count_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts")]
pub struct CycleCountListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts/table")]
pub struct CycleCountTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts/create")]
pub struct CycleCountCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts/{id}")]
pub struct CycleCountDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(CycleCountListPath::PATH, get(wms_cycle_count_list::get_cycle_count_list))
        .route(CycleCountTablePath::PATH, get(wms_cycle_count_list::get_cycle_count_table))
        .route(CycleCountCreatePath::PATH, get(wms_cycle_count_create::get_cycle_count_create).post(wms_cycle_count_create::create_cycle_count))
        .route(CycleCountDetailPath::PATH, get(wms_cycle_count_detail::get_cycle_count_detail))
}
