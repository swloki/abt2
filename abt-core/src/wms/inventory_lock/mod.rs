pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{CreateLockReq, InventoryLock, LockFilter};
pub use service::InventoryLockService;

use sqlx::PgPool;

pub fn new_inventory_lock_service(pool: PgPool) -> impl InventoryLockService {
    implt::InventoryLockServiceImpl::new(pool)
}
