use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::sales_order_list;
use crate::pages::sales_order_create;
use crate::pages::sales_order_detail;
use crate::pages::sales_order_edit;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders")]
pub struct OrderListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/create")]
pub struct OrderCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/products")]
pub struct OrderProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/item-row")]
pub struct OrderItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/customer-contacts")]
pub struct OrderCustomerContactsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}")]
pub struct OrderDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}/edit-form")]
pub struct OrderEditFormPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}/update")]
#[allow(dead_code)]
pub struct UpdateOrderPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}/delete")]
pub struct DeleteOrderPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}/confirm")]
pub struct ConfirmOrderPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}/complete")]
pub struct CompleteOrderPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/orders/{id}/cancel")]
pub struct CancelOrderPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(OrderListPath::PATH, get(sales_order_list::get_order_list))
.route(OrderCreatePath::PATH, get(sales_order_create::get_order_create).post(sales_order_create::create_order))
        .route(OrderProductsPath::PATH, get(sales_order_create::get_products))
        .route(OrderItemRowPath::PATH, get(sales_order_create::get_order_item_row))
        .route(OrderCustomerContactsPath::PATH, get(sales_order_create::get_customer_contacts))
        .route(OrderDetailPath::PATH, get(sales_order_detail::get_order_detail))
        .route(OrderEditFormPath::PATH, get(sales_order_edit::get_order_edit).post(sales_order_edit::update_order))
        .route(DeleteOrderPath::PATH, post(sales_order_list::delete_order))
        .route(ConfirmOrderPath::PATH, post(sales_order_detail::confirm_order))
        .route(CompleteOrderPath::PATH, post(sales_order_detail::complete_order))
        .route(CancelOrderPath::PATH, post(sales_order_detail::cancel_order))
}
