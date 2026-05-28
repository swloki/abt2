pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use repo::WarehouseExportRow;
pub use service::WarehouseService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_warehouse_service(pool: PgPool) -> impl WarehouseService {
    use implt::WarehouseServiceImpl;
    WarehouseServiceImpl::new(Arc::new(pool))
}
