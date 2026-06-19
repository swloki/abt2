pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::GlPeriodService;

use sqlx::PgPool;

pub fn new_gl_period_service(pool: PgPool) -> impl GlPeriodService {
    implt::GlPeriodServiceImpl::new(pool)
}
