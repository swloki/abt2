use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_bin_list, wms_bin_create, wms_bin_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/bins")]
pub struct BinListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/bins/table")]
pub struct BinTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/bins/create")]
pub struct BinCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/bins/{id}")]
pub struct BinDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(BinListPath::PATH, get(wms_bin_list::get_bin_list))
        .route(BinTablePath::PATH, get(wms_bin_list::get_bin_table))
        .route(BinCreatePath::PATH, get(wms_bin_create::get_bin_create).post(wms_bin_create::create_bin))
        .route(BinDetailPath::PATH, get(wms_bin_detail::get_bin_detail))
}
