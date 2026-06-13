use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::mes_demand_pool;
use crate::pages::mes_demand_pool_create;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/demand-pool")]
pub struct MesDemandPoolListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/demand-pool/create")]
pub struct MesDemandPoolCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/demand-pool/demand-rows")]
pub struct MesDemandRowsPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            MesDemandPoolListPath::PATH,
            get(mes_demand_pool::get_demand_pool_list),
        )
        .route(
            MesDemandPoolCreatePath::PATH,
            get(mes_demand_pool_create::get_demand_pool_create)
                .post(mes_demand_pool_create::create_plan_from_demands),
        )
        .route(
            MesDemandRowsPath::PATH,
            get(mes_demand_pool::get_demand_rows),
        )
}
