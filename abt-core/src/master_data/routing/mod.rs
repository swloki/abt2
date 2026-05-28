pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::RoutingService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_routing_service(pool: PgPool) -> impl RoutingService {
    use implt::RoutingServiceImpl;
    use repo::RoutingRepo;
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;

    let pool = Arc::new(pool);
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool));

    RoutingServiceImpl::new(RoutingRepo, audit, event_bus)
}
