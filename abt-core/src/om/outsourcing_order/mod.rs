pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

use sqlx::PgPool;

use service::OutsourcingOrderService;

pub fn new_outsourcing_order_service(pool: PgPool) -> impl OutsourcingOrderService {
    implt::OutsourcingOrderServiceImpl::new(pool)
}
