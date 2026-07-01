pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::SupplierService;

use sqlx::PgPool;

pub fn new_supplier_service(pool: PgPool) -> impl SupplierService {
    implt::SupplierServiceImpl::new(pool)
}
