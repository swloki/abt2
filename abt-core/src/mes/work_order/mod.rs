pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::WorkOrderService;

use sqlx::PgPool;

pub fn new_work_order_service(pool: PgPool) -> impl WorkOrderService {
    implt::WorkOrderServiceImpl::new(pool)
}
