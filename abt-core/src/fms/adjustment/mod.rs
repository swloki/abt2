pub mod enums;
pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use enums::*;
pub use model::*;
pub use service::AdjustmentService;

use sqlx::PgPool;

pub fn new_adjustment_service(pool: PgPool) -> impl AdjustmentService {
    implt::AdjustmentServiceImpl::new(pool)
}
