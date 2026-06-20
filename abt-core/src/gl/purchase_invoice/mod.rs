pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::PurchaseInvoiceService;

use sqlx::PgPool;

pub fn new_purchase_invoice_service(pool: PgPool) -> impl PurchaseInvoiceService {
    implt::PurchaseInvoiceServiceImpl::new(pool)
}
