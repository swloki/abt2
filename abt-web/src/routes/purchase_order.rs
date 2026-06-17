use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_order_list;
use crate::pages::purchase_order_create;
use crate::pages::purchase_order_detail;
use crate::pages::purchase_order_edit;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders")]
pub struct POListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/create")]
pub struct POCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/products")]
pub struct POProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/create/item-row")]
pub struct POItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/create/supplier-detail")]
pub struct POSupplierDetailPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}")]
pub struct PODetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/confirm")]
pub struct POConfirmPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/cancel")]
pub struct POCancelPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/delete")]
pub struct PODeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/orders/{id}/edit")]
pub struct POEditPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(POListPath::PATH, get(purchase_order_list::get_po_list))
.route(POCreatePath::PATH, get(purchase_order_create::get_po_create).post(purchase_order_create::create_po))
                .route(POItemRowPath::PATH, get(purchase_order_create::get_po_item_row))
        .route(POSupplierDetailPath::PATH, get(purchase_order_create::get_po_supplier_detail))
        .route("/admin/purchase/orders/tax-rates", get(purchase_order_create::get_tax_rates))
        .route(PODetailPath::PATH, get(purchase_order_detail::get_po_detail))
        .route(POConfirmPath::PATH, post(purchase_order_detail::confirm_po))
        .route(POCancelPath::PATH, post(purchase_order_detail::cancel_po))
        .route("/admin/purchase/orders/{id}/submit", post(purchase_order_detail::submit_po))
        .route("/admin/purchase/orders/{id}/approve", post(purchase_order_detail::approve_po_order))
        .route("/admin/purchase/orders/{id}/reject", post(purchase_order_detail::reject_po))
        .route("/admin/purchase/orders/{id}/items/update", post(purchase_order_detail::update_po_items))
        .route("/admin/purchase/orders/merge", post(purchase_order_detail::merge_po))
        .route(POEditPath::PATH, get(purchase_order_edit::get_po_edit).post(purchase_order_edit::update_po))
}
