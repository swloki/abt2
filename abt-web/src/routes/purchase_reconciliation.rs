use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::*;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/reconciliations")]
pub struct PreconListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/reconciliations/table")]
pub struct PreconTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/reconciliations/create")]
pub struct PreconCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/reconciliations/{id}")]
pub struct PreconDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/reconciliations/{id}/confirm")]
pub struct PreconConfirmPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(PreconListPath::PATH, get(purchase_recon_list::get_precon_list))
        .route(PreconTablePath::PATH, get(purchase_recon_list::get_precon_table))
        .route(PreconCreatePath::PATH, get(purchase_recon_create::get_precon_create).post(purchase_recon_create::create_precon))
        .route(PreconDetailPath::PATH, get(purchase_recon_detail::get_precon_detail))
        .route(PreconConfirmPath::PATH, post(purchase_recon_detail::confirm_precon))
}
