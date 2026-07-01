pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::PurchaseReconciliationService;

use sqlx::PgPool;

pub fn new_purchase_reconciliation_service(pool: PgPool) -> impl PurchaseReconciliationService {
    implt::PurchaseReconciliationServiceImpl::new(pool)
}
