use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::misc_request_list;
use crate::pages::misc_request_create;
use crate::pages::misc_request_detail;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/misc-requests")]
pub struct MiscListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/misc-requests/table")]
pub struct MiscTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/misc-requests/create")]
pub struct MiscCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/misc-requests/{id}")]
pub struct MiscDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/misc-requests/{id}/approve")]
pub struct MiscApprovePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/misc-requests/{id}/cancel")]
pub struct MiscCancelPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(MiscListPath::PATH, get(misc_request_list::get_misc_list))
        .route(MiscTablePath::PATH, get(misc_request_list::get_misc_table))
        .route(MiscCreatePath::PATH, get(misc_request_create::get_misc_create).post(misc_request_create::create_misc))
        .route(MiscDetailPath::PATH, get(misc_request_detail::get_misc_detail))
        .route(MiscApprovePath::PATH, post(misc_request_detail::approve_misc))
        .route(MiscCancelPath::PATH, post(misc_request_detail::cancel_misc))
}
