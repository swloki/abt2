pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use service::PurchaseOrderService;

use sqlx::PgPool;

pub fn new_purchase_order_service(pool: PgPool) -> impl PurchaseOrderService {
    implt::PurchaseOrderServiceImpl::new(pool)
}
