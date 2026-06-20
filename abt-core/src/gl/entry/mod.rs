pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;

use sqlx::PgPool;

pub fn new_gl_entry_service(pool: PgPool) -> impl GlEntryService {
    implt::GlEntryServiceImpl::new(pool)
}

// Re-export the service trait
pub use service::GlEntryService;
