pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::ProductService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_product_service(pool: PgPool) -> impl ProductService {
    use implt::ProductServiceImpl;
    use repo::ProductRepo;
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;

    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool, event_bus.clone()));

    ProductServiceImpl::new(ProductRepo, doc_seq, audit, event_bus, state_machine)
}
