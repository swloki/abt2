pub mod model;
pub mod implt;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::NotificationService;

use sqlx::postgres::PgPool;

pub fn new_notification_service(pool: PgPool) -> impl NotificationService {
    implt::NotificationServiceImpl::new(pool)
}
