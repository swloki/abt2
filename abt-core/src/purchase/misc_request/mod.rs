pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use service::MiscellaneousRequestService;

use sqlx::PgPool;

pub fn new_misc_request_service(pool: PgPool) -> impl MiscellaneousRequestService {
    implt::MiscellaneousRequestServiceImpl::new(pool)
}
