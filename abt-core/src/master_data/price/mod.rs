pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::ProductPriceService;

use sqlx::PgPool;

pub fn new_product_price_service(pool: PgPool) -> impl ProductPriceService {
    implt::PriceServiceImpl::new(pool)
}
