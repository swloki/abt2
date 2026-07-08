pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::BomOperationService;

use sqlx::PgPool;

pub fn new_bom_operation_service(pool: PgPool) -> impl BomOperationService {
    implt::BomOperationServiceImpl::new(pool)
}
