pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ShippingRequestService;

use std::sync::Arc;
use sqlx::PgPool;

pub fn new_shipping_request_service(pool: PgPool) -> impl ShippingRequestService {
    use implt::ShippingRequestServiceImpl;
    use repo::{ShippingRequestItemRepo, ShippingRequestRepo};
    use crate::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo};
    use crate::sales::sales_order::{new_sales_order_service, SalesOrderService};
    use crate::shared::audit_log::implt::AuditLogServiceImpl;
    use crate::shared::cost_entry::implt::CostEntryServiceImpl;
    use crate::shared::document_link::implt::DocumentLinkServiceImpl;
    use crate::shared::document_sequence::implt::DocumentSequenceServiceImpl;
    use crate::shared::event_bus::implt::DomainEventBusImpl;
    use crate::shared::idempotency::implt::IdempotencyServiceImpl;
    use crate::shared::inventory_reservation::implt::InventoryReservationServiceImpl;
    use crate::shared::state_machine::implt::StateMachineServiceImpl;
    use crate::qms::inspection_result::implt::InspectionResultServiceImpl;
    use crate::qms::inspection_result::service::InspectionResultService;
    use crate::qms::inspection_specification::implt::InspectionSpecificationServiceImpl;

    let sales_order_svc: Arc<dyn SalesOrderService> = Arc::new(new_sales_order_service(pool.clone()));
    let pool = Arc::new(pool);
    let doc_seq = Arc::new(DocumentSequenceServiceImpl::new(pool.clone()));
    let audit = Arc::new(AuditLogServiceImpl::new(pool.clone()));
    let event_bus = Arc::new(DomainEventBusImpl::new(pool.clone()));
    let state_machine = Arc::new(StateMachineServiceImpl::new(pool.clone(), event_bus.clone()));
    let doc_link = Arc::new(DocumentLinkServiceImpl::new(pool.clone()));
    let inv_res = Arc::new(InventoryReservationServiceImpl::new(pool.clone()));
    let cost_entry = Arc::new(CostEntryServiceImpl::new(pool.clone()));
    let idempotency = Arc::new(IdempotencyServiceImpl::new(pool.clone()));
    let spec_service = Arc::new(InspectionSpecificationServiceImpl::new(
        pool.clone(),
        doc_seq.clone(),
        state_machine.clone(),
        audit.clone(),
    ));
    let qms: Arc<dyn InspectionResultService> = Arc::new(InspectionResultServiceImpl::new(
        pool.clone(),
        doc_seq.clone(),
        state_machine.clone(),
        event_bus.clone(),
        audit.clone(),
        idempotency,
        spec_service,
    ));

    ShippingRequestServiceImpl::new(
        ShippingRequestRepo,
        ShippingRequestItemRepo,
        SalesOrderRepo,
        SalesOrderItemRepo,
        doc_seq,
        state_machine,
        audit,
        event_bus,
        sales_order_svc,
        doc_link,
        inv_res,
        cost_entry,
        qms,
    )
}
