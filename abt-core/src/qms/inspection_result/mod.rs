pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::InspectionResultService;

use sqlx::PgPool;

pub fn new_inspection_result_service(pool: PgPool) -> impl InspectionResultService {
    implt::InspectionResultServiceImpl::new(pool)
}
