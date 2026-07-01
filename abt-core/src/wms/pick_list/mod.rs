pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::PickListService;

use sqlx::PgPool;

pub fn new_pick_list_service(pool: PgPool) -> impl PickListService {
    implt::PickListServiceImpl::new(pool)
}
