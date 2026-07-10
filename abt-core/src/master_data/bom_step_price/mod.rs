pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::BomStepPriceService;

use sqlx::PgPool;

pub fn new_bom_step_price_service(pool: PgPool) -> impl BomStepPriceService {
    implt::BomStepPriceServiceImpl::new(pool)
}
