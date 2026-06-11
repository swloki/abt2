use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::shipping_list;
use crate::pages::shipping_detail;
use crate::pages::shipping_create;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping")]
pub struct ShippingListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/table")]
pub struct ShippingTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/create")]
pub struct ShippingCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}/edit")]
pub struct ShippingEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/draft")]
pub struct ShippingSaveDraftPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}")]
pub struct ShippingDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}/delete")]
pub struct ShippingDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}/confirm")]
pub struct ConfirmShippingPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}/pick")]
pub struct PickShippingPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}/ship")]
pub struct ShipShippingPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/{id}/cancel")]
pub struct CancelShippingPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/customer-contacts")]
pub struct ShippingCustomerContactsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/shipping/order-search")]
pub struct ShippingOrderSearchPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ShippingListPath::PATH, get(shipping_list::get_shipping_list))
        .route(ShippingTablePath::PATH, get(shipping_list::get_shipping_table))
        .route(ShippingDetailPath::PATH, get(shipping_detail::get_shipping_detail))
        .route(ShippingCreatePath::PATH, get(shipping_create::get_shipping_create).post(shipping_create::post_shipping_create))
        .route(ShippingEditPath::PATH, get(shipping_create::get_shipping_edit))
        .route(ShippingSaveDraftPath::PATH, post(shipping_create::post_save_draft))
        .route(ShippingDeletePath::PATH, post(shipping_list::delete_shipping))
        .route(ShippingCustomerContactsPath::PATH, get(shipping_create::get_customer_contacts))
        .route(ShippingOrderSearchPath::PATH, get(shipping_create::get_order_search))
        .route(ConfirmShippingPath::PATH, post(shipping_detail::confirm_shipping))
        .route(PickShippingPath::PATH, post(shipping_detail::pick_shipping))
        .route(ShipShippingPath::PATH, post(shipping_detail::ship_shipping))
        .route(CancelShippingPath::PATH, post(shipping_detail::cancel_shipping))
}
