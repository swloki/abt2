pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::RoutingService;

use sqlx::PgPool;

pub fn new_routing_service(pool: PgPool) -> impl RoutingService {
    implt::RoutingServiceImpl::new(pool)
}
