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
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/delete")]
pub struct OrderRoutingDeletePath {
    pub order_id: i64,
    pub routing_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/edit")]
pub struct OrderRoutingEditPath {
    pub order_id: i64,
    pub routing_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/load-from-template")]
pub struct OrderRoutingLoadTemplatePath {
    pub order_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/{order_id}/routings/load-from-recent")]
pub struct OrderRoutingLoadRecentPath {
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
        .route(OrderRoutingDeletePath::PATH, post(mes_order_detail::delete_routing))
        .route(OrderRoutingEditPath::PATH, get(mes_order_detail::get_routing_edit).post(mes_order_detail::post_routing_edit))
        .route(OrderRoutingLoadTemplatePath::PATH, post(mes_order_detail::load_routings_from_template))
        .route(OrderRoutingLoadRecentPath::PATH, post(mes_order_detail::load_routings_from_recent))
        .route(SourceOrderSearchPath::PATH, get(mes_order_create::search_source_orders))
        .route(SourcePlanSearchPath::PATH, get(mes_order_create::search_source_plans))
}
