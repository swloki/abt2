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
#[typed_path("/admin/wms/stock/zones")]
pub struct StockZonesPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock/detail")]
pub struct StockDetailPath;

#[derive(Debug, Deserialize)]
pub struct StockDetailQuery {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockListPath::PATH, get(wms_stock_list::get_stock_list))
        .route(StockZonesPath::PATH, get(wms_stock_list::get_zone_options))
        .route(StockDetailPath::PATH, get(wms_stock_list::get_stock_detail))
}
