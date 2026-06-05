use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_backflush_list, wms_backflush_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/backflushes")]
pub struct BackflushListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/backflushes/table")]
pub struct BackflushTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/backflushes/{id}")]
pub struct BackflushDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(BackflushListPath::PATH, get(wms_backflush_list::get_backflush_list))
        .route(BackflushTablePath::PATH, get(wms_backflush_list::get_backflush_table))
        .route(BackflushDetailPath::PATH, get(wms_backflush_detail::get_backflush_detail))
}
