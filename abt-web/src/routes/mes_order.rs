use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_order_list, mes_order_create, mes_order_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders")]
pub struct OrderListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/create")]
pub struct OrderCreatePath;


#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{id}")]
pub struct OrderDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/release")]
pub struct OrderReleasePath {
    pub order_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/close")]
pub struct OrderClosePath {
    pub order_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/cancel")]
pub struct OrderCancelPath {
    pub order_id: i64,
}
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/unrelease")]
pub struct OrderUnreleasePath {
    pub order_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/split")]
pub struct OrderSplitPath {
    pub order_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/mes/source-orders/search")]
pub struct SourceOrderSearchPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/mes/source-plans/search")]
pub struct SourcePlanSearchPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(OrderListPath::PATH, get(mes_order_list::get_order_list))
        .route(
            OrderCreatePath::PATH,
            get(mes_order_create::get_order_create).post(mes_order_create::create_order),
        )

        .route(OrderDetailPath::PATH, get(mes_order_detail::get_order_detail))
        .route(OrderReleasePath::PATH, post(mes_order_detail::release_order))
        .route(OrderClosePath::PATH, post(mes_order_detail::close_order))
        .route(OrderCancelPath::PATH, post(mes_order_detail::cancel_order))
        .route(OrderUnreleasePath::PATH, post(mes_order_detail::unrelease_order))
        .route(OrderSplitPath::PATH, post(mes_order_detail::split_order))
        .route(SourceOrderSearchPath::PATH, get(mes_order_create::search_source_orders))
        .route(SourcePlanSearchPath::PATH, get(mes_order_create::search_source_plans))
}
