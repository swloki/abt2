use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::*;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations")]
pub struct PQListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/create")]
pub struct PQCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/products")]
pub struct PQProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/create/item-row")]
pub struct PQItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/create/supplier-contacts")]
pub struct PQSupplierContactsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/{id}")]
pub struct PQDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/{id}/activate")]
pub struct PQActivatePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/{id}/cancel")]
pub struct PQCancelPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/{id}/delete")]
pub struct PQDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/quotations/{id}/convert")]
pub struct PQConvertPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(PQListPath::PATH, get(purchase_quotation_list::get_pq_list))
.route(PQCreatePath::PATH, get(purchase_quotation_create::get_pq_create).post(purchase_quotation_create::create_pq))
                .route(PQItemRowPath::PATH, get(purchase_quotation_create::get_pq_item_row))
        .route(PQSupplierContactsPath::PATH, get(purchase_quotation_create::get_pq_supplier_contacts))
        .route(PQDetailPath::PATH, get(purchase_quotation_detail::get_pq_detail))
        .route(PQDeletePath::PATH, post(purchase_quotation_detail::delete_pq))
        .route(PQActivatePath::PATH, post(purchase_quotation_detail::activate_pq))
        .route(PQCancelPath::PATH, post(purchase_quotation_detail::cancel_pq))
        .route(PQConvertPath::PATH, post(purchase_quotation_detail::convert_pq))
}
