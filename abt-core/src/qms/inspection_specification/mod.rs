pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use service::InspectionSpecificationService;

use sqlx::PgPool;

pub fn new_inspection_specification_service(pool: PgPool) -> impl InspectionSpecificationService {
    implt::InspectionSpecificationServiceImpl::new(pool)
}
