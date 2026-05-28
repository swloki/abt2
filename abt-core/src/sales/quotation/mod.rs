pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::QuotationService;

use sqlx::PgPool;

pub fn new_quotation_service(pool: PgPool) -> impl QuotationService {
    implt::QuotationServiceImpl::new(pool)
}
