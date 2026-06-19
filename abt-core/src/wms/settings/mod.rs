pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{UpdateWmsSettingsReq, WmsSettings};
pub use service::WmsSettingsService;

use sqlx::PgPool;

pub fn new_wms_settings_service(pool: PgPool) -> impl WmsSettingsService {
    implt::WmsSettingsServiceImpl::new(pool)
}
