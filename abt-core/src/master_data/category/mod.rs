pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::CategoryService;

use sqlx::PgPool;

pub fn new_category_service(pool: PgPool) -> impl CategoryService {
    implt::CategoryServiceImpl::new(pool)
}
