pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::MaterialRequisitionService;

use sqlx::PgPool;

pub fn new_material_requisition_service(pool: PgPool) -> impl MaterialRequisitionService {
    implt::MaterialRequisitionServiceImpl::new(pool)
}
