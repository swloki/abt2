use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{routing_list, routing_create, routing_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings")]
pub struct RoutingListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/table")]
pub struct RoutingTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/new")]
pub struct RoutingCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}")]
pub struct RoutingDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/bom-table")]
pub struct RoutingBomTablePath {
    pub id: i64,
}


#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}")]
pub struct RoutingUpdatePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/delete")]
pub struct RoutingDeletePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(RoutingListPath::PATH, get(routing_list::get_routing_list))
        .route(
            RoutingTablePath::PATH,
            get(routing_list::get_routing_table),
        )
        .route(
            RoutingCreatePath::PATH,
            get(routing_create::get_routing_create).post(routing_create::post_routing_create),
        )
        .route(
            RoutingDetailPath::PATH,
            get(routing_detail::get_routing_detail),
        )
        .route(
            RoutingBomTablePath::PATH,
            get(routing_detail::get_routing_bom_table),
        )
        .route(
            RoutingUpdatePath::PATH,
            post(routing_detail::update_routing),
        )
        .route(
            RoutingDeletePath::PATH,
            post(routing_list::delete_routing),
        )
}
