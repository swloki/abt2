//! 采购 SRM 模块

pub mod enums;
pub mod arrival_handler;
pub mod demand_handler;
pub mod misc_request;
pub use arrival_handler::ArrivalAcceptedHandler;
pub mod order;
pub mod payment;
pub mod quotation;
pub mod reconciliation;
pub mod return_order;

pub use misc_request::MiscellaneousRequestService;
pub use order::PurchaseOrderService;
pub use payment::PaymentRequestService;
pub use quotation::PurchaseQuotationService;
pub use reconciliation::PurchaseReconciliationService;
pub use return_order::PurchaseReturnService;
