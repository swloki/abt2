use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_dashboard;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase")]
pub struct PurchaseDashboardPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(PurchaseDashboardPath::PATH, get(purchase_dashboard::get_purchase_dashboard))
}
