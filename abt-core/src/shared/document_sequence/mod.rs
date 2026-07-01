pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::DocumentSequence;
pub use service::DocumentSequenceService;

use sqlx::PgPool;

pub fn new_document_sequence_service(pool: PgPool) -> impl DocumentSequenceService {
    implt::DocumentSequenceServiceImpl::new(pool)
}
