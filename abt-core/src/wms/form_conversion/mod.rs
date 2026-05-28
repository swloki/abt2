pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{
    ConversionFilter, ConversionItem, CreateConversionItemReq, CreateConversionReq,
    FormConversion,
};
pub use service::FormConversionService;

use sqlx::PgPool;

pub fn new_form_conversion_service(pool: PgPool) -> impl FormConversionService {
    implt::FormConversionServiceImpl::new(pool)
}
