pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{PurchaseSettings, UpdatePurchaseSettingsRequest};
pub use service::PurchaseSettingsService;

pub fn new_purchase_settings_service(pool: sqlx::PgPool) -> impl PurchaseSettingsService {
    implt::PurchaseSettingsServiceImpl::new(pool)
}
