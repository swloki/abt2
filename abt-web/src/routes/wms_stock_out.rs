use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_stock_out_list, wms_stock_out_create, wms_stock_out_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out")]
pub struct StockOutListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create")]
pub struct StockOutCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create/products")]
pub struct StockOutProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create/item-row")]
pub struct StockOutItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create/confirm-shipping")]
pub struct StockOutConfirmShippingPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create/confirm-requisition")]
pub struct StockOutConfirmReqPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/create/suggest-bins")]
pub struct StockOutSuggestBinsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/stock-out/{id}")]
pub struct StockOutDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(StockOutListPath::PATH, get(wms_stock_out_list::get_stock_out_list))
        .route(StockOutItemRowPath::PATH, get(wms_stock_out_create::get_item_row))
        .route(StockOutConfirmShippingPath::PATH, post(wms_stock_out_create::confirm_shipping))
        .route(StockOutConfirmReqPath::PATH, post(wms_stock_out_create::confirm_requisition))
        .route(StockOutSuggestBinsPath::PATH, get(wms_stock_out_create::suggest_bins))
        .route(StockOutCreatePath::PATH, get(wms_stock_out_create::get_stock_out_create).post(wms_stock_out_create::create_stock_out))
        .route(StockOutDetailPath::PATH, get(wms_stock_out_detail::get_stock_out_detail))
}
