use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::*;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/payments")]
pub struct PayListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/payments/create")]
pub struct PayCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/payments/{id}")]
pub struct PayDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/payments/{id}/approve")]
pub struct PayApprovePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/payments/{id}/cancel")]
pub struct PayCancelPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/payments/supplier-info")]
pub struct PaySupplierInfoPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(PayListPath::PATH, get(payment_request_list::get_pay_list))
.route(PaySupplierInfoPath::PATH, get(payment_request_create::get_supplier_info))
        .route(PayCreatePath::PATH, get(payment_request_create::get_pay_create).post(payment_request_create::create_pay))
        .route(PayDetailPath::PATH, get(payment_request_detail::get_pay_detail))
        .route(PayApprovePath::PATH, post(payment_request_detail::approve_pay))
        .route(PayCancelPath::PATH, post(payment_request_detail::cancel_pay))
}
