use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_cascade_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cascade")]
pub struct CascadeListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cascade/table")]
pub struct CascadeTablePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(CascadeListPath::PATH, get(wms_cascade_list::get_cascade_list))
        .route(CascadeTablePath::PATH, get(wms_cascade_list::get_cascade_table))
}
