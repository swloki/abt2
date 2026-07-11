pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::AttachmentService;

use sqlx::PgPool;

/// 通用附件服务工厂（按需获取，用完即弃）。
pub fn new_attachment_service(pool: PgPool) -> impl AttachmentService {
    implt::AttachmentServiceImpl::new(pool)
}
