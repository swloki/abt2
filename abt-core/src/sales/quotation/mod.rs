pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::QuotationService;

use std::sync::Arc;
use sqlx::PgPool;

use crate::master_data::customer::CustomerService;

pub fn new_quotation_service(pool: PgPool) -> impl QuotationService {
    use implt::QuotationServiceImpl;
    use repo::{QuotationItemRepo, QuotationRepo};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;

    let customer_svc: Arc<dyn CustomerService> =
        Arc::new(crate::master_data::customer::new_customer_service(pool.clone()));

    let pool = Arc::new(pool);
    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool, event_bus.clone()));

    QuotationServiceImpl::new(
        QuotationRepo,
        QuotationItemRepo,
        doc_seq,
        state_machine,
        audit,
        event_bus,
        customer_svc,
    )
}
