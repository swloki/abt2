use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_conversion_list, wms_conversion_create, wms_conversion_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/conversions")]
pub struct ConversionListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/conversions/table")]
pub struct ConversionTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/conversions/create")]
pub struct ConversionCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/conversions/{id}")]
pub struct ConversionDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ConversionListPath::PATH, get(wms_conversion_list::get_conversion_list))
        .route(ConversionTablePath::PATH, get(wms_conversion_list::get_conversion_table))
        .route(ConversionCreatePath::PATH, get(wms_conversion_create::get_conversion_create).post(wms_conversion_create::create_conversion))
        .route(ConversionDetailPath::PATH, get(wms_conversion_detail::get_conversion_detail))
}
