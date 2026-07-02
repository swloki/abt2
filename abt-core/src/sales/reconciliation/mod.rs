pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::ReconciliationService;

use sqlx::PgPool;

pub fn new_reconciliation_service(pool: PgPool) -> impl ReconciliationService {
    use implt::ReconciliationServiceImpl;

    ReconciliationServiceImpl::new(pool)
}
