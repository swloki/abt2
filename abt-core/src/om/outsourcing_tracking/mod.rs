pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::OutsourcingTrackingService;
pub use model::{OutsourcingTracking, OverdueTrackingQuery, RecordNodeReq};

pub fn new_outsourcing_tracking_service() -> impl OutsourcingTrackingService {
    implt::OutsourcingTrackingServiceImpl::new()
}
