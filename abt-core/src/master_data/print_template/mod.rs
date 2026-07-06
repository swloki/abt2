pub mod model;
pub(crate) mod repo;
pub mod service;
pub mod implt;
pub mod mock_data;

pub use model::*;
pub use service::PrintTemplateService;
pub use mock_data::mock_context;

use sqlx::PgPool;

pub fn new_print_template_service(_pool: PgPool) -> impl PrintTemplateService {
    implt::PrintTemplateServiceImpl::new()
}
