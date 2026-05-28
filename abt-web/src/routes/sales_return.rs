use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::sales_return_list;
use crate::pages::sales_return_detail;
use crate::pages::sales_return_create;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns")]
pub struct ReturnListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/table")]
pub struct ReturnTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/new")]
pub struct ReturnCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/orders")]
pub struct ReturnOrdersPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}")]
pub struct ReturnDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}/confirm")]
pub struct ConfirmReturnPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}/receive")]
pub struct ReceiveReturnPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}/inspect")]
pub struct InspectReturnPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}/complete")]
pub struct CompleteReturnPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}/reject")]
pub struct RejectReturnPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/returns/{id}/delete")]
pub struct ReturnDeletePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ReturnListPath::PATH, get(sales_return_list::get_return_list))
        .route(ReturnTablePath::PATH, get(sales_return_list::get_return_table))
        .route(ReturnCreatePath::PATH, get(sales_return_create::get_return_create).post(sales_return_create::create_return))
        .route(ReturnOrdersPath::PATH, get(sales_return_create::get_orders))
        .route(ReturnDetailPath::PATH, get(sales_return_detail::get_return_detail))
        .route(ConfirmReturnPath::PATH, post(sales_return_detail::confirm_return))
        .route(ReceiveReturnPath::PATH, post(sales_return_detail::receive_return))
        .route(InspectReturnPath::PATH, post(sales_return_detail::inspect_return))
        .route(CompleteReturnPath::PATH, post(sales_return_detail::complete_return))
        .route(RejectReturnPath::PATH, post(sales_return_detail::reject_return))
        .route(ReturnDeletePath::PATH, post(sales_return_list::delete_return))
}
