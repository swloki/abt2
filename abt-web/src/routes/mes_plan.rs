use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_plan_list, mes_plan_create, mes_plan_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans")]
pub struct PlanListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/create")]
pub struct PlanCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/create/item-row")]
pub struct PlanItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/{id}")]
pub struct PlanDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/{plan_id}/confirm")]
pub struct PlanConfirmPath {
    pub plan_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/plans/{plan_id}/release")]
pub struct PlanReleasePath {
    pub plan_id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(PlanListPath::PATH, get(mes_plan_list::get_plan_list))
        .route(PlanItemRowPath::PATH, get(mes_plan_create::get_item_row))
        .route(
            PlanCreatePath::PATH,
            get(mes_plan_create::get_plan_create).post(mes_plan_create::create_plan),
        )
        .route(PlanDetailPath::PATH, get(mes_plan_detail::get_plan_detail))
        .route(
            PlanConfirmPath::PATH,
            post(mes_plan_detail::confirm_plan),
        )
        .route(
            PlanReleasePath::PATH,
            post(mes_plan_detail::release_plan),
        )
}
