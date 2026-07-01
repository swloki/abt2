pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{AuditLog, AuditLogQuery, RecordAuditLogReq};
pub use service::AuditLogService;

use sqlx::PgPool;

pub fn new_audit_log_service(pool: PgPool) -> impl AuditLogService {
    implt::AuditLogServiceImpl::new(pool)
}
