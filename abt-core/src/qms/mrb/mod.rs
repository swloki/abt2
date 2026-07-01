pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::MrbService;

use sqlx::PgPool;

pub fn new_mrb_service(pool: PgPool) -> impl MrbService {
    implt::MrbServiceImpl::new(pool)
}
