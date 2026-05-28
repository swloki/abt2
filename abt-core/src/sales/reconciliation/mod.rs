pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ReconciliationService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_reconciliation_service(pool: PgPool) -> impl ReconciliationService {
    use implt::ReconciliationServiceImpl;
    use repo::{ReconciliationItemRepo, ReconciliationRepo};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::cost_entry::implt::CostEntryServiceImpl;
    use crate::shared::cost_entry::service::CostEntryService;
    use crate::shared::document_link::implt::DocumentLinkServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;
    use crate::shared::idempotency::implt::IdempotencyServiceImpl;
    use crate::shared::idempotency::service::IdempotencyService;
    use crate::fms::cash_journal::implt::CashJournalServiceImpl;
    use crate::fms::cash_journal::service::CashJournalService;

    let pool = Arc::new(pool);
    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
    let doc_link = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));
    let cost_entry: Arc<dyn CostEntryService> = Arc::new(CostEntryServiceImpl::new(pool.clone()));
    let idempotency: Arc<dyn IdempotencyService> = Arc::new(IdempotencyServiceImpl::new(pool.clone()));
    let cash_journal: Arc<dyn CashJournalService> = Arc::new(CashJournalServiceImpl::new(
        doc_seq.clone(),
        state_machine.clone(),
        audit.clone(),
        event_bus.clone(),
        idempotency,
    ));

    ReconciliationServiceImpl::new(
        ReconciliationRepo,
        ReconciliationItemRepo,
        doc_seq,
        state_machine,
        audit,
        doc_link,
        cost_entry,
        cash_journal,
    )
}
