pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ShippingRequestService;

use sqlx::PgPool;

pub fn new_shipping_request_service(pool: PgPool) -> impl ShippingRequestService {
    implt::ShippingRequestServiceImpl::new(pool)
}
