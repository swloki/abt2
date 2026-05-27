use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::sidebar;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/sidebar/body/{module}")]
pub struct SidebarBodyPath {
    pub module: String,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(SidebarBodyPath::PATH, get(sidebar::get_sidebar_body))
}
