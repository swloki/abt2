pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::PaymentRequestService;

use sqlx::PgPool;

pub fn new_payment_request_service(pool: PgPool) -> impl PaymentRequestService {
    implt::PaymentRequestServiceImpl::new(pool)
}
