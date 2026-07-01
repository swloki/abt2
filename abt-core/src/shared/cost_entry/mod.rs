pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{CostEntry, EntryRequest};
pub use service::CostEntryService;

use sqlx::PgPool;

pub fn new_cost_entry_service(pool: PgPool) -> impl CostEntryService {
    implt::CostEntryServiceImpl::new(pool)
}
