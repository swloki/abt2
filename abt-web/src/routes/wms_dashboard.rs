use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_dashboard;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms")]
pub struct WmsDashboardPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(WmsDashboardPath::PATH, get(wms_dashboard::get_wms_dashboard))
}
