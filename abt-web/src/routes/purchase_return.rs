use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_return_list;
use crate::pages::purchase_return_create;
use crate::pages::purchase_return_detail;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns")]
pub struct PRListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns/table")]
pub struct PRTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns/create")]
pub struct PRCreatePath;
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns/order-items")]
pub struct PROrderItemsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns/{id}")]
pub struct PRDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns/{id}/confirm")]
pub struct PRConfirmPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/returns/{id}/cancel")]
pub struct PRCancelPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(PRListPath::PATH, get(purchase_return_list::get_pr_list))
        .route(PRTablePath::PATH, get(purchase_return_list::get_pr_table))
        .route(PRCreatePath::PATH, get(purchase_return_create::get_pr_create).post(purchase_return_create::create_pr))
        .route(PROrderItemsPath::PATH, get(purchase_return_create::get_pr_order_items))
        .route(PRDetailPath::PATH, get(purchase_return_detail::get_pr_detail))
        .route(PRConfirmPath::PATH, post(purchase_return_detail::confirm_pr))
        .route(PRCancelPath::PATH, post(purchase_return_detail::cancel_pr))
}
