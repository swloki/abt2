use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::mes_dashboard;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes")]
pub struct MesDashboardPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(MesDashboardPath::PATH, get(mes_dashboard::get_mes_dashboard))
}
