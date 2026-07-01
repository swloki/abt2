pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::WorkCenterService;

use sqlx::PgPool;

pub fn new_work_center_service(pool: PgPool) -> impl WorkCenterService {
    implt::WorkCenterServiceImpl::new(pool)
}
