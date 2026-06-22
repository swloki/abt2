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
#[typed_path("/admin/wms/stock-in/create")]
pub struct StockInCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/products")]
pub struct StockInProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/item-row")]
pub struct StockInItemRowPath;
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/source-pick")]
pub struct StockInSourcePickPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/source-items")]
pub struct StockInSourceItemsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/confirm-pos")]
pub struct StockInConfirmPosPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/suggest-bins")]
pub struct StockInSuggestBinsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/create/confirm-wo")]
pub struct StockInConfirmWoPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-in/{id}")]
pub struct StockInDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockInListPath::PATH, get(wms_stock_in_list::get_stock_in_list))
        .route(StockInSourcePickPath::PATH, get(wms_stock_in_create::get_source_pick))
        .route(StockInSourceItemsPath::PATH, get(wms_stock_in_create::get_source_items))
        .route(StockInConfirmPosPath::PATH, post(wms_stock_in_create::confirm_purchase_orders))
        .route(StockInSuggestBinsPath::PATH, get(wms_stock_in_create::suggest_bins))
        .route(StockInConfirmWoPath::PATH, post(wms_stock_in_create::confirm_work_order))
        .route(StockInItemRowPath::PATH, get(wms_stock_in_create::get_item_row))
        .route(StockInCreatePath::PATH, get(wms_stock_in_create::get_stock_in_create).post(wms_stock_in_create::create_stock_in))
        .route(StockInDetailPath::PATH, get(wms_stock_in_detail::get_stock_in_detail))
}
