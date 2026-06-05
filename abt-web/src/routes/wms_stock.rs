use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_stock_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock")]
pub struct StockListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock/table")]
pub struct StockTablePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockListPath::PATH, get(wms_stock_list::get_stock_list))
        .route(StockTablePath::PATH, get(wms_stock_list::get_stock_table))
}
