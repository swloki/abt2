pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::TaxRate;
pub use service::TaxRateService;

pub fn new_tax_rate_service(pool: sqlx::PgPool) -> impl TaxRateService {
    implt::TaxRateServiceImpl::new(pool)
}
