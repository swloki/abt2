use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_low_stock_list;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/low-stock")]
pub struct LowStockListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/low-stock/{id}/ack")]
pub struct LowStockAckPath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(LowStockListPath::PATH, get(wms_low_stock_list::get_low_stock_list))
        .route(LowStockAckPath::PATH, post(wms_low_stock_list::ack_alert))
}
