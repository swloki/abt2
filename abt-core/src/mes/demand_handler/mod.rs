//! MES 需求池子模块

pub mod handler;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use handler::MesDemandCreatedHandler;
pub use model::*;
pub use service::MesDemandService;

use sqlx::postgres::PgPool;

pub fn new_mes_demand_service(pool: PgPool) -> impl MesDemandService {
    implt::MesDemandServiceImpl::new(pool)
}
