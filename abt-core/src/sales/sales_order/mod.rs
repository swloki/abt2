pub mod event_handlers;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use event_handlers::{SalesDemandConfirmedHandler, SalesDemandRejectedHandler, SalesDemandReleasedHandler};
pub use model::*;
pub use service::{SalesOrderService, ReplenishmentAllocationStrategy, AllocationResult, DemandService};

use sqlx::PgPool;

pub fn new_sales_order_service(pool: PgPool) -> impl SalesOrderService {
    implt::SalesOrderServiceImpl::new(pool)
}

pub fn new_demand_service(pool: PgPool) -> impl DemandService {
    implt::DemandServiceImpl::new(pool)
}
