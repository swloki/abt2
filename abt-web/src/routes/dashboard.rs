use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::dashboard;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin")]
pub struct DashboardPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(DashboardPath::PATH, get(dashboard::get_dashboard))
}
