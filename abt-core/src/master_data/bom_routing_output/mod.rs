pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::BomRoutingOutputService;

use sqlx::PgPool;

pub fn new_bom_routing_output_service(pool: PgPool) -> impl BomRoutingOutputService {
    implt::BomRoutingOutputServiceImpl::new(pool)
}
