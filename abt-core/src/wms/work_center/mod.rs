pub mod implt;
pub mod model;
pub mod service;

pub use model::*;
pub use service::WorkCenterService;

use sqlx::PgPool;

pub fn new_work_center_service(pool: PgPool) -> impl WorkCenterService {
    implt::WorkCenterServiceImpl::new(pool)
}
