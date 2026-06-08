pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::WriteOffService;

use sqlx::PgPool;

pub fn new_write_off_service(pool: PgPool) -> impl WriteOffService {
    implt::WriteOffServiceImpl::new(pool)
}
