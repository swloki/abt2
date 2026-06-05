use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_stock_in_list, wms_stock_in_create};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in")]
pub struct StockInListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/table")]
pub struct StockInTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create")]
pub struct StockInCreatePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockInListPath::PATH, get(wms_stock_in_list::get_stock_in_list))
        .route(StockInTablePath::PATH, get(wms_stock_in_list::get_stock_in_table))
        .route(StockInCreatePath::PATH, get(wms_stock_in_create::get_stock_in_create).post(wms_stock_in_create::create_stock_in))
}
