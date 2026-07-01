pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::PurchaseReturnService;

use sqlx::PgPool;

pub fn new_purchase_return_service(pool: PgPool) -> impl PurchaseReturnService {
    implt::PurchaseReturnServiceImpl::new(pool)
}
