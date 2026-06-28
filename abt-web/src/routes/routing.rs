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
#[typed_path("/admin/md/routings/new")]
pub struct RoutingCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}")]
pub struct RoutingDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/edit")]
pub struct RoutingEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/copy")]
pub struct RoutingCopyPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/delete")]
pub struct RoutingDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/boms")]
pub struct RoutingBomListPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/bom/bind")]
pub struct RoutingBindBomPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/bom/unbind")]
pub struct RoutingUnbindBomPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(RoutingListPath::PATH, get(routing_list::get_routing_list))
        .route(
            RoutingCreatePath::PATH,
            get(routing_create::get_routing_create).post(routing_create::post_routing_create),
        )
        .route(
            RoutingDetailPath::PATH,
            get(routing_detail::get_routing_detail),
        )
        .route(
            RoutingEditPath::PATH,
            get(routing_create::get_routing_edit).post(routing_create::post_routing_update),
        )
        .route(RoutingCopyPath::PATH, get(routing_create::get_routing_copy))
        .route(
            RoutingDeletePath::PATH,
            post(routing_list::delete_routing),
        )
        .route(RoutingBomListPath::PATH, get(routing_detail::get_routing_bom_list))
        .route(RoutingBindBomPath::PATH, post(routing_detail::bind_bom))
        .route(RoutingUnbindBomPath::PATH, post(routing_detail::unbind_bom))
}
