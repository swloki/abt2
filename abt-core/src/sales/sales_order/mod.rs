pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::{SalesOrderService, ReplenishmentAllocationStrategy, AllocationResult};

use sqlx::PgPool;

pub fn new_sales_order_service(pool: PgPool) -> impl SalesOrderService {
    implt::SalesOrderServiceImpl::new(pool)
}
