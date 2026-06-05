pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::StrategyService;

use sqlx::PgPool;

pub fn new_strategy_service(pool: PgPool) -> impl StrategyService {
    implt::StrategyServiceImpl::new(pool)
}