pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::WorkCalendarService;

use sqlx::PgPool;

pub fn new_work_calendar_service(pool: PgPool) -> impl WorkCalendarService {
    implt::WorkCalendarServiceImpl::new(pool)
}
