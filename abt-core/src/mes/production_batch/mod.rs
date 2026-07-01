pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::ProductionBatchService;

use sqlx::PgPool;

pub fn new_production_batch_service(pool: PgPool) -> impl ProductionBatchService {
    implt::ProductionBatchServiceImpl::new(pool)
}
