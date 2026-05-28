pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{DocumentLink, LinkRequest};
pub use service::DocumentLinkService;

use sqlx::PgPool;

pub fn new_document_link_service(pool: PgPool) -> impl DocumentLinkService {
    implt::DocumentLinkServiceImpl::new(pool)
}
