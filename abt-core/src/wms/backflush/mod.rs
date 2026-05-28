pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{BackflushFilter, BackflushItem, BackflushRecord, CreateBackflushItemReq, CreateBackflushReq};
pub use service::BackflushService;

use sqlx::PgPool;

pub fn new_backflush_service(pool: PgPool) -> impl BackflushService {
    implt::BackflushServiceImpl::new(pool)
}
