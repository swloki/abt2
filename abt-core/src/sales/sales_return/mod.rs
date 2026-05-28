pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::SalesReturnService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_sales_return_service(pool: PgPool) -> impl SalesReturnService {
    use implt::SalesReturnServiceImpl;
    use repo::{SalesReturnItemRepo, SalesReturnRepo};
    use crate::sales::sales_order::repo::SalesOrderItemRepo;
    use crate::sales::shipping_request::{new_shipping_request_service, ShippingRequestService};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::cost_entry::implt::CostEntryServiceImpl;
    use crate::shared::cost_entry::service::CostEntryService;
    use crate::shared::document_link::implt::DocumentLinkServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;
    use crate::qms::rma::implt::RmaServiceImpl;
    use crate::qms::rma::service::RmaService;

    let pool = Arc::new(pool);
    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
    let shipping_svc: Arc<dyn ShippingRequestService> =
        Arc::new(new_shipping_request_service(pool.as_ref().clone()));
    let doc_link = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));
    let cost_entry: Arc<dyn CostEntryService> = Arc::new(CostEntryServiceImpl::new(pool.clone()));
    let rma: Arc<dyn RmaService> = Arc::new(RmaServiceImpl::new(
        pool.clone(),
        doc_seq.clone(),
        state_machine.clone(),
        event_bus.clone(),
        audit.clone(),
        doc_link.clone(),
    ));

    SalesReturnServiceImpl::new(
        SalesReturnRepo,
        SalesReturnItemRepo,
        SalesOrderItemRepo,
        doc_seq,
        state_machine,
        audit,
        event_bus,
        shipping_svc,
        doc_link,
        cost_entry,
        rma,
    )
}
