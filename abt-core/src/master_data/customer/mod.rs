pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::CustomerService;

use sqlx::PgPool;

pub fn new_customer_service(pool: PgPool) -> impl CustomerService {
    implt::CustomerServiceImpl::new(pool)
}
