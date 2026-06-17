use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_arrival_list, wms_arrival_create, wms_arrival_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals")]
pub struct ArrivalListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals/create")]
pub struct ArrivalCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals/create/products")]
pub struct ArrivalProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals/create/item-row")]
pub struct ArrivalItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals/create/po-pick")]
pub struct ArrivalPoPickPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals/create/po-items/{po_id}")]
pub struct ArrivalPoItemsPath {
    pub po_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/arrivals/{id}")]
pub struct ArrivalDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ArrivalListPath::PATH, get(wms_arrival_list::get_arrival_list))
                .route(ArrivalItemRowPath::PATH, get(wms_arrival_create::get_item_row))
        .route(ArrivalPoPickPath::PATH, get(wms_arrival_create::get_po_pick))
        .route(ArrivalPoItemsPath::PATH, get(wms_arrival_create::get_po_items))
        .route(ArrivalCreatePath::PATH, get(wms_arrival_create::get_arrival_create).post(wms_arrival_create::create_arrival))
        .route(ArrivalDetailPath::PATH, get(wms_arrival_detail::get_arrival_detail).post(wms_arrival_detail::post_arrival_action))
}
