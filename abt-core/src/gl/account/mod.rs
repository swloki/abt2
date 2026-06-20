pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::{GlAccountService, GlAccountNode};

use sqlx::PgPool;

pub fn new_gl_account_service(pool: PgPool) -> impl GlAccountService {
    implt::GlAccountServiceImpl::new(pool)
}
