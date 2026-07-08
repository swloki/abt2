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

/// 编辑页关联 BOM drawer 分页端点（Issue #212：drawer 内分页，每页 10 条）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/bound-boms")]
pub struct RoutingBoundBomsPath {
    pub id: i64,
}

/// 详情页关联 BOM 的「产出/计件覆盖层」编辑分区（GET，drawer body 用）。
/// 产出品/计件价已从 `routing_steps` 模板下沉到 per-BOM 覆盖层 `bom_routing_outputs`，
/// 在此按 BOM（product_code）维护。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/outputs")]
pub struct RoutingOutputEditPath {
    pub id: i64,
}

/// UPSERT 单道工序的产出覆盖（by product_code + step_order）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/outputs/upsert")]
pub struct RoutingOutputUpsertPath {
    pub id: i64,
}

/// 删除单道工序的产出覆盖（回退到模板默认）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/routings/{id}/outputs/delete")]
pub struct RoutingOutputDeletePath {
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
        .route(
            RoutingBoundBomsPath::PATH,
            get(routing_create::get_routing_bound_boms),
        )
        .route(
            RoutingOutputEditPath::PATH,
            get(routing_detail::get_routing_output_edit),
        )
        .route(
            RoutingOutputUpsertPath::PATH,
            post(routing_detail::upsert_routing_output),
        )
        .route(
            RoutingOutputDeletePath::PATH,
            post(routing_detail::delete_routing_output),
        )
}
