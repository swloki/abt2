pub mod model;
pub mod service;
pub mod repo;
pub mod implt;

pub use service::MesDashboardService;
pub use implt::MesDashboardServiceImpl;

use sqlx::postgres::PgPool;

pub fn new_mes_dashboard_service(pool: PgPool) -> impl MesDashboardService {
    MesDashboardServiceImpl::new(pool)
}
