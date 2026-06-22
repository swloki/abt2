pub mod enums;
pub mod implt;
pub mod model;
pub mod payment_terms;
pub mod repo;
pub mod service;

pub use enums::*;
pub use model::*;
pub use service::ArApService;

use sqlx::PgPool;

pub fn new_ar_ap_service(pool: PgPool) -> impl ArApService {
    implt::ArApServiceImpl::new(pool)
}
