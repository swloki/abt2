pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::CostAccountingService;

pub fn new_cost_accounting_service() -> impl CostAccountingService {
    implt::CostAccountingServiceImpl::new()
}
