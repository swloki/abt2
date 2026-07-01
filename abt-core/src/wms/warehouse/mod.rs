pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use repo::WarehouseExportRow;
pub use service::WarehouseService;

use sqlx::PgPool;

pub fn new_warehouse_service(pool: PgPool) -> impl WarehouseService {
    use implt::WarehouseServiceImpl;
    WarehouseServiceImpl::new(pool)
}
