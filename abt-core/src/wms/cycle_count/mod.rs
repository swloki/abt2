pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{
    CountCycleCountReq, CountItemReq, CreateCycleCountItemReq, CreateCycleCountReq,
    CycleCount, CycleCountFilter, CycleCountItem,
};
pub use service::CycleCountService;

use sqlx::PgPool;

pub fn new_cycle_count_service(pool: PgPool) -> impl CycleCountService {
    implt::CycleCountServiceImpl::new(pool)
}
