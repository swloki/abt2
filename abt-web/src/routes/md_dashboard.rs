use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::md_dashboard;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md")]
pub struct MdDashboardPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(
        MdDashboardPath::PATH,
        get(md_dashboard::get_md_dashboard),
    )
}
