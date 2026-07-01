pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::ProductWatcherService;

use sqlx::PgPool;

pub fn new_product_watcher_service(pool: PgPool) -> impl ProductWatcherService {
    let _ = pool;
    implt::ProductWatcherServiceImpl::new()
}
