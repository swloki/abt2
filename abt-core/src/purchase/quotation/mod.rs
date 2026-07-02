pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::PurchaseQuotationService;

use sqlx::PgPool;

pub fn new_purchase_quotation_service(pool: PgPool) -> impl PurchaseQuotationService {
    implt::PurchaseQuotationServiceImpl::new(pool)
}
