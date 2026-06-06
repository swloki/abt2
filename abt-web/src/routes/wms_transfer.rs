use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_transfer_list, wms_transfer_create, wms_transfer_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers")]
pub struct TransferListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/table")]
pub struct TransferTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/create")]
pub struct TransferCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/create/products")]
pub struct TransferProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/create/item-row")]
pub struct TransferItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/{id}")]
pub struct TransferDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(TransferListPath::PATH, get(wms_transfer_list::get_transfer_list))
        .route(TransferTablePath::PATH, get(wms_transfer_list::get_transfer_table))
        .route(TransferProductsPath::PATH, get(wms_transfer_create::get_products))
        .route(TransferItemRowPath::PATH, get(wms_transfer_create::get_item_row))
        .route(TransferCreatePath::PATH, get(wms_transfer_create::get_transfer_create).post(wms_transfer_create::create_transfer))
        .route(TransferDetailPath::PATH, get(wms_transfer_detail::get_transfer_detail).post(wms_transfer_detail::post_transfer_action))
}
