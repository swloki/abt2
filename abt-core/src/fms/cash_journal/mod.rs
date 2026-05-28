pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::CashJournalService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_cash_journal_service(pool: PgPool) -> impl CashJournalService {
    use implt::CashJournalServiceImpl;
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::idempotency::implt::IdempotencyServiceImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;

    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let event_bus = Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let idempotency = Arc::new(IdempotencyServiceImpl::new(pool.clone()));

    CashJournalServiceImpl::new(doc_seq, state_machine, audit, event_bus, idempotency)
}
