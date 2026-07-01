pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::ProductionReceiptService;

use sqlx::PgPool;

pub fn new_production_receipt_service(pool: PgPool) -> impl ProductionReceiptService {
    implt::ProductionReceiptServiceImpl::new(pool)
}
