//! 采购需求池子模块

pub mod handler;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use handler::PurchaseDemandCreatedHandler;
pub use model::*;
pub use service::PurchaseDemandService;

use sqlx::postgres::PgPool;

pub fn new_purchase_demand_service(pool: PgPool) -> impl PurchaseDemandService {
    implt::PurchaseDemandServiceImpl::new(pool)
}
