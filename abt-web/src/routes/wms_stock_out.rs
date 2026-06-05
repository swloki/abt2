use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_stock_out_list, wms_stock_out_create};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out")]
pub struct StockOutListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/table")]
pub struct StockOutTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create")]
pub struct StockOutCreatePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockOutListPath::PATH, get(wms_stock_out_list::get_stock_out_list))
        .route(StockOutTablePath::PATH, get(wms_stock_out_list::get_stock_out_table))
        .route(StockOutCreatePath::PATH, get(wms_stock_out_create::get_stock_out_create).post(wms_stock_out_create::create_stock_out))
}
