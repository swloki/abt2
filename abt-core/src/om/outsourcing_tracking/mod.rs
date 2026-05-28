pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use service::OutsourcingTrackingService;

use sqlx::PgPool;

pub fn new_outsourcing_tracking_service(pool: PgPool) -> impl OutsourcingTrackingService {
    implt::OutsourcingTrackingServiceImpl::new()
}
