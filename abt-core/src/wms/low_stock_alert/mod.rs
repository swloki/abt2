pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{LowStockAlert, LowStockAlertFilter};
pub use service::LowStockAlertService;

use sqlx::PgPool;

pub fn new_low_stock_alert_service(pool: PgPool) -> impl LowStockAlertService {
    implt::LowStockAlertServiceImpl::new(pool)
}
