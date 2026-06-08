pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::ExpenseReimbursementService;

use sqlx::PgPool;

pub fn new_expense_service(pool: PgPool) -> impl ExpenseReimbursementService {
    implt::ExpenseReimbursementServiceImpl::new(pool)
}
