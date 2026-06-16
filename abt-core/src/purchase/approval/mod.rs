pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::{PurchaseApprovalRule, RuleUpsertRequest};
pub use service::PurchaseApprovalService;

pub fn new_approval_service(pool: sqlx::PgPool) -> impl PurchaseApprovalService {
    implt::PurchaseApprovalServiceImpl::new(pool)
}
