pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::GlMappingService;

use sqlx::PgPool;

pub fn new_gl_mapping_service(pool: PgPool) -> impl GlMappingService {
    implt::GlMappingServiceImpl::new(pool)
}
