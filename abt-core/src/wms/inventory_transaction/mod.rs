pub mod implt;
pub mod model;
pub(crate) mod repo;
pub mod service;

pub use model::{InventoryTransaction, RecordTransactionReq, TransactionFilter};
pub use service::InventoryTransactionService;

use sqlx::PgPool;

pub fn new_inventory_transaction_service(pool: PgPool) -> impl InventoryTransactionService {
    implt::InventoryTransactionServiceImpl::new(pool)
}
