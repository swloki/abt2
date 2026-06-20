pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::SalesInvoiceService;

use sqlx::PgPool;

pub fn new_sales_invoice_service(pool: PgPool) -> impl SalesInvoiceService {
    implt::SalesInvoiceServiceImpl::new(pool)
}
