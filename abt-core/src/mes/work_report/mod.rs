pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::WorkReportService;

use sqlx::PgPool;

pub fn new_work_report_service(pool: PgPool) -> impl WorkReportService {
    implt::WorkReportServiceImpl::new(pool)
}
