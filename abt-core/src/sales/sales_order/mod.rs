pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::SalesOrderService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_sales_order_service(pool: PgPool) -> impl SalesOrderService {
    use implt::SalesOrderServiceImpl;
    use repo::{SalesOrderItemRepo, SalesOrderRepo};
    use crate::master_data::customer::{new_customer_service, CustomerService};
    use crate::sales::quotation::{new_quotation_service, QuotationService};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::document_link::implt::DocumentLinkServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::inventory_reservation::implt::InventoryReservationServiceImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;

    let customer_svc: Arc<dyn CustomerService> = Arc::new(new_customer_service(pool.clone()));
    let quotation_svc: Arc<dyn QuotationService> = Arc::new(new_quotation_service(pool.clone()));
    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus: Arc<dyn crate::shared::event_bus::service::DomainEventBus> =
        Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
    let inv_res = Arc::new(InventoryReservationServiceImpl::new(pool.clone()));
    let doc_link = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));

    SalesOrderServiceImpl::new(
        SalesOrderRepo,
        SalesOrderItemRepo,
        doc_seq,
        state_machine,
        audit,
        event_bus,
        customer_svc,
        quotation_svc,
        doc_link,
        inv_res,
    )
}
