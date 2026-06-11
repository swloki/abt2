use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_lock_list, wms_lock_create, wms_lock_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/locks")]
pub struct LockListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/locks/create")]
pub struct LockCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/locks/{id}")]
pub struct LockDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(LockListPath::PATH, get(wms_lock_list::get_lock_list))
        .route(LockCreatePath::PATH, get(wms_lock_create::get_lock_create).post(wms_lock_create::create_lock))
        .route(LockDetailPath::PATH, get(wms_lock_detail::get_lock_detail).post(wms_lock_detail::post_lock_action))
}
