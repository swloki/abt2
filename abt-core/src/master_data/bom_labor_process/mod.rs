pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::BomLaborProcessService;

use sqlx::PgPool;

pub fn new_bom_labor_process_service(pool: PgPool) -> impl BomLaborProcessService {
    implt::BomLaborProcessServiceImpl::new(pool)
}
