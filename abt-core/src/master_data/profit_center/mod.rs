pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::ProfitCenterService;

use sqlx::PgPool;

pub fn new_profit_center_service(pool: PgPool) -> impl ProfitCenterService {
    implt::ProfitCenterServiceImpl::new(pool)
}
