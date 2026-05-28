pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::IdempotencyRecord;
pub use service::IdempotencyService;

use sqlx::PgPool;

pub fn new_idempotency_service(pool: PgPool) -> impl IdempotencyService {
    implt::IdempotencyServiceImpl::new(pool)
}
