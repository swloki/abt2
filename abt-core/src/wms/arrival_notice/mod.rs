pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ArrivalNoticeService;

use sqlx::PgPool;

pub fn new_arrival_notice_service(pool: PgPool) -> impl ArrivalNoticeService {
    implt::ArrivalNoticeServiceImpl::new(pool)
}
