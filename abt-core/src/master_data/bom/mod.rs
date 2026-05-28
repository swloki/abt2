pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::{BomCategoryService, BomCommandService, BomCostService, BomNodeService, BomQueryService};

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_bom_query_service(pool: PgPool) -> impl BomQueryService {
    use implt::BomQueryServiceImpl;
    use repo::{BomRepo, BomNodeRepo, BomSnapshotRepo};

    BomQueryServiceImpl::new(BomRepo, BomNodeRepo, BomSnapshotRepo)
}

pub fn new_bom_command_service(pool: PgPool) -> impl BomCommandService {
    use implt::BomCommandServiceImpl;
    use repo::{BomRepo, BomNodeRepo, BomSnapshotRepo};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;

    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool, event_bus.clone()));

    BomCommandServiceImpl::new(
        BomRepo,
        BomNodeRepo,
        BomSnapshotRepo,
        doc_seq,
        audit,
        event_bus,
        state_machine,
    )
}

pub fn new_bom_node_service(pool: PgPool) -> impl BomNodeService {
    use implt::BomNodeServiceImpl;
    use repo::{BomRepo, BomNodeRepo};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;

    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool));

    BomNodeServiceImpl::new(BomRepo, BomNodeRepo, audit, event_bus)
}
