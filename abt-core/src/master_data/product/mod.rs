pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::ProductService;

use sqlx::PgPool;

pub fn new_product_service(pool: PgPool) -> impl ProductService {
    implt::ProductServiceImpl::new(pool)
}
