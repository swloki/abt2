pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::LaborProcessDictService;

use sqlx::PgPool;

pub fn new_labor_process_dict_service(pool: PgPool) -> impl LaborProcessDictService {
    implt::LaborProcessDictServiceImpl::new(pool)
}
