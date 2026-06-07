pub mod model;
pub mod service;
pub mod repo;
pub mod implt;

pub use service::ProductionExceptionService;
pub use implt::ProductionExceptionServiceImpl;

use sqlx::postgres::PgPool;

pub fn new_production_exception_service(pool: PgPool) -> impl ProductionExceptionService {
    ProductionExceptionServiceImpl::new(pool)
}
