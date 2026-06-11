use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_strategy_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/strategies")]
pub struct StrategyListPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StrategyListPath::PATH, get(wms_strategy_list::get_strategy_list))
}
