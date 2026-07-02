pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::*;
pub use service::CashJournalService;

use sqlx::PgPool;

pub fn new_cash_journal_service(pool: PgPool) -> impl CashJournalService {
    implt::CashJournalServiceImpl::new(pool)
}
