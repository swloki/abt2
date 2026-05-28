pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ProductionPlanService;

use sqlx::PgPool;

pub fn new_production_plan_service(pool: PgPool) -> impl ProductionPlanService {
    implt::ProductionPlanServiceImpl::new(pool)
}
