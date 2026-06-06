use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_stock_in_list, wms_stock_in_create, wms_stock_in_detail};
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

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/products")]
pub struct StockInProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/item-row")]
pub struct StockInItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/{id}")]
pub struct StockInDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockInListPath::PATH, get(wms_stock_in_list::get_stock_in_list))
        .route(StockInTablePath::PATH, get(wms_stock_in_list::get_stock_in_table))
        .route(StockInProductsPath::PATH, get(wms_stock_in_create::get_products))
        .route(StockInItemRowPath::PATH, get(wms_stock_in_create::get_item_row))
        .route(StockInCreatePath::PATH, get(wms_stock_in_create::get_stock_in_create).post(wms_stock_in_create::create_stock_in))
        .route(StockInDetailPath::PATH, get(wms_stock_in_detail::get_stock_in_detail))
}
