use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::mes_demand_pool;
use crate::pages::mes_demand_pool_create;
use crate::state::AppState;

// ── Typed Paths ──
// MesDemandPoolListPath（独立列表页 /admin/mes/demand-pool）已下线，需求池收口到
// 作业中心（/admin/mes/work-center）的 demand card。保留 create / demand-rows 子端点：
// 作业中心复用（创建工单 + 物料行展开懒加载需求明细）。

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
            MesDemandPoolCreatePath::PATH,
            get(mes_demand_pool_create::get_demand_pool_create)
                .post(mes_demand_pool_create::create_plan_from_demands),
        )
        .route(MesDemandRowsPath::PATH, get(mes_demand_pool::get_demand_rows))
}
