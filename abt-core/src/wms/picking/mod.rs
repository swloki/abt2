pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{
    CreatePickingItemReq, CreatePickingReq, DoneItemReq, PickingFilter, StockPicking,
    StockPickingItem,
};
pub use service::PickingService;

use sqlx::PgPool;

pub fn new_picking_service(pool: PgPool) -> impl PickingService {
    implt::PickingServiceImpl::new(pool)
}
