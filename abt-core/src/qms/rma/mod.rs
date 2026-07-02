pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::RmaService;

use sqlx::PgPool;

pub fn new_rma_service(pool: PgPool) -> impl RmaService {
    implt::RmaServiceImpl::new(pool)
}
