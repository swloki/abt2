pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::SalesReturnService;

use sqlx::PgPool;

pub fn new_sales_return_service(pool: PgPool) -> impl SalesReturnService {
    implt::SalesReturnServiceImpl::new(pool)
}
