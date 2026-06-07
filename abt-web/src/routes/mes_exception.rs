use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_exception_list, mes_exception_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/exceptions")]
pub struct ExceptionListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/exceptions/table")]
pub struct ExceptionTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/exceptions/{id}")]
pub struct ExceptionDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ExceptionListPath::PATH, get(mes_exception_list::get_exception_list))
        .route(ExceptionTablePath::PATH, get(mes_exception_list::get_exception_table))
        .route(ExceptionDetailPath::PATH, get(mes_exception_detail::get_exception_detail))
}
