pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ProductionInspectionService;

use sqlx::PgPool;

pub fn new_production_inspection_service(pool: PgPool) -> impl ProductionInspectionService {
    implt::ProductionInspectionServiceImpl::new(pool)
}
