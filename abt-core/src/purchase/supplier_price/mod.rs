pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::SupplierProductPrice;
pub use service::SupplierPriceService;

pub fn new_supplier_price_service(pool: sqlx::PgPool) -> impl SupplierPriceService {
    implt::SupplierPriceServiceImpl::new(pool)
}
